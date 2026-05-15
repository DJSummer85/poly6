//! Bot Orchestrator - Manages multiple trading bots
//!
//! Coordinates bot execution, session tracking, and portfolio management

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;

use crate::db::Db;
use crate::db::queries;
use crate::db::BotRecord;
use crate::trading::bot_executor::strategies::{Signal, StrategyExecutor, StrategyContext};
use crate::trading::risk_manager::RiskManager;
use crate::trading::bot_loss_tracker::BotLossTrackerManager;
use crate::trading::strategy_coordinator::StrategyCoordinator;
use crate::api::market::fetch_active_markets;
use crate::api::CachedCredentials;
use crate::trading::polymarket::{PolymarketClient, OrderRequest};

#[derive(Debug, Clone, serde::Serialize)]
pub enum BotEvent {
    SessionStarted { bot_id: i64, session_id: i64, bot_name: String },
    SessionEnded { bot_id: i64, session_id: i64, final_balance: f64, total_pnl: f64 },
    TradeDecision { bot_id: i64, bot_name: String, outcome: String, confidence: f64, bet_size: f64, reason: String },
    OrderExecuted { bot_id: i64, order_id: String },
    BalanceUpdated { bot_id: i64, balance: f64 },
    MarketTransition { new_market_slug: String },
    Error { bot_id: i64, message: String },
    Scanning { bot_id: i64, market_slug: String },
    Evaluating { bot_id: i64, strategy: String, confidence: f64 },
    PositionUpdate { bot_id: i64, side: String, size: f64, price: f64, unrealized_pnl: f64 },
    TradeResult { bot_id: i64, won: bool, pnl: f64 },
}

#[derive(Debug, Clone)]
pub struct PendingBet {
    pub side: String,
    pub bet_size: f64,
    pub start_price: f64,   // BTC ár fogadás nyitásakor
    pub entry_price: f64,   // YES/NO token ára (0-1)
    pub decision_id: i64,
    pub price_to_beat: Option<f64>, // Polymarket settlement küszöb
    pub market_end_time: i64,       // Mikor zár a piac (unix timestamp)
}

#[derive(Debug, Clone)]
pub struct RunningBot {
    pub bot_id: i64,
    pub bot_name: String,
    pub session_id: i64,
    pub user_id: i64,
    pub strategy: StrategyExecutor,
    pub last_market_slug: Option<String>,
    pub consecutive_errors: u32,
    pub last_btc_price: Option<f64>,
    pub btc_window_open: Option<f64>,
    pub current_balance: f64,
    pub pending_bet: Option<PendingBet>,
    pub btc_price_history: Vec<(f64, Instant)>,
    pub last_trade_time: Option<Instant>,
}

#[derive(Clone)]
pub struct BotOrchestrator {
    db: Db,
    running_bots: Arc<RwLock<HashMap<i64, RunningBot>>>,
    event_sender: mpsc::UnboundedSender<BotEvent>,
    pub auto_save_interval: Duration,
    pub risk_manager: Arc<RwLock<RiskManager>>,
    pub loss_tracker: Arc<RwLock<BotLossTrackerManager>>,
    pub coordinator: Arc<RwLock<StrategyCoordinator>>,
    pub telegram_service: Option<Arc<crate::services::telegram::TelegramService>>,
}

/// Restore running bots from database on startup
pub async fn restore_running_bots(orchestrator: Arc<BotOrchestrator>) {
    tracing::info!("Restoring running bots from database...");

    let running_sessions = match queries::get_all_running_sessions(&orchestrator.db).await {
        Ok(sessions) => sessions,
        Err(e) => {
            tracing::error!("Failed to fetch running sessions for restore: {}", e);
            return;
        }
    };

    if running_sessions.is_empty() {
        tracing::info!("No running bots to restore");
        return;
    }

    tracing::info!("Found {} running sessions to restore", running_sessions.len());

    for session in &running_sessions {
        let bot = match queries::get_bot_by_id(&orchestrator.db, session.bot_id, session.user_id).await {
            Ok(Some(b)) => b,
            Ok(None) => {
                tracing::warn!("Bot {} not found for session {}, skipping restore", session.bot_id, session.id);
                continue;
            }
            Err(e) => {
                tracing::warn!("Failed to fetch bot {} for restore: {}", session.bot_id, e);
                continue;
            }
        };

        let strategy = StrategyExecutor::new(&bot.strategy_type, &bot.params);

        let running_bot = RunningBot {
            bot_id: bot.id,
            bot_name: bot.name.clone(),
            session_id: session.id,
            user_id: session.user_id,
            strategy,
            last_market_slug: None,
            consecutive_errors: 0,
            last_btc_price: None,
            btc_window_open: None,
            current_balance: session.start_balance,
            pending_bet: None,
            btc_price_history: Vec::new(),
            last_trade_time: None,
        };

        {
            let mut running = orchestrator.running_bots.write().await;
            running.insert(bot.id, running_bot);
        }

        tracing::info!("Restored bot {} (session {}) with balance {:.2}", bot.id, session.id, session.start_balance);

        let orchestrator_clone = orchestrator.clone();
        let bot_id = bot.id;
        let session_user_id = session.user_id;
        tokio::spawn(async move {
            start_orchestrator_loop(
                orchestrator_clone,
                bot_id,
                session_user_id,
                5,
                None,
            ).await;
        });
    }

    tracing::info!("Finished restoring {} running bots", running_sessions.len());
}

impl BotOrchestrator {
    pub fn new(db: Db, event_sender: mpsc::UnboundedSender<BotEvent>) -> Self {
        Self {
            db,
            running_bots: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            auto_save_interval: Duration::from_secs(30),
            risk_manager: Arc::new(RwLock::new(RiskManager::new_paper())),
            loss_tracker: Arc::new(RwLock::new(BotLossTrackerManager::new())),
            coordinator: Arc::new(RwLock::new(StrategyCoordinator::default_with_config())),
            telegram_service: None,
        }
    }

    pub fn with_telegram(mut self, service: Arc<crate::services::telegram::TelegramService>) -> Self {
        self.telegram_service = Some(service);
        self
    }

    pub async fn resume_bot(&self, bot: &BotRecord, current_balance: f64) -> Result<i64, String> {
        let mut running = self.running_bots.write().await;
        if running.contains_key(&bot.id) { return Ok(0); }
        let strategy = StrategyExecutor::new(&bot.strategy_type, &bot.params);
        let session_id = queries::create_session(&self.db, bot.id, bot.user_id, current_balance, Some(bot.params.as_str()), &bot.trading_mode).await.unwrap_or(0);
        running.insert(bot.id, RunningBot {
            bot_id: bot.id, bot_name: bot.name.clone(), session_id, user_id: bot.user_id, strategy, last_market_slug: None,
            consecutive_errors: 0, last_btc_price: None, btc_window_open: None, current_balance,
            pending_bet: None, btc_price_history: Vec::new(),
            last_trade_time: None,
        });
        self.event_sender.send(BotEvent::SessionStarted { bot_id: bot.id, session_id, bot_name: bot.name.clone() }).ok();
        Ok(session_id)
    }

    pub async fn start_bot(&self, bot: &BotRecord, initial_balance: f64) -> Result<i64, String> {
        let running = self.running_bots.read().await;
        if running.contains_key(&bot.id) { return Err("Bot is already running".to_string()); }
        drop(running);
        queries::update_bot_status(&self.db, bot.id, bot.user_id, "running").await.ok();
        let session_id = queries::create_session(&self.db, bot.id, bot.user_id, initial_balance, Some(bot.params.as_str()), &bot.trading_mode).await.map_err(|e| e.to_string())?;
        let mut running = self.running_bots.write().await;
        running.insert(bot.id, RunningBot {
            bot_id: bot.id, bot_name: bot.name.clone(), session_id, user_id: bot.user_id, strategy: StrategyExecutor::new(&bot.strategy_type, &bot.params), 
            last_market_slug: None, consecutive_errors: 0, last_btc_price: None, btc_window_open: None, 
            current_balance: initial_balance, pending_bet: None, btc_price_history: Vec::new(),
            last_trade_time: None,
        });
        self.event_sender.send(BotEvent::SessionStarted { bot_id: bot.id, session_id, bot_name: bot.name.clone() }).ok();
        tracing::info!("Bot {} started (session {}), trading_mode={}", bot.id, session_id, bot.trading_mode);
        Ok(session_id)
    }

    pub async fn stop_bot(&self, bot_id: i64, user_id: i64) -> Result<(), String> {
        let mut running = self.running_bots.write().await;
        if let Some(rb) = running.remove(&bot_id) {
            queries::update_bot_status(&self.db, bot_id, user_id, "stopped").await.ok();
            self.event_sender.send(BotEvent::SessionEnded { bot_id, session_id: rb.session_id, final_balance: 0.0, total_pnl: 0.0 }).ok();
        }
        Ok(())
    }

    pub async fn execute_cycle(&self, bot_id: i64, user_id: i64, credential_cache: Option<Arc<RwLock<HashMap<i64, CachedCredentials>>>>) -> Result<(), String> {
        let running_bot = { let running = self.running_bots.read().await; running.get(&bot_id).cloned() };
        let mut rb = if let Some(b) = running_bot {
            b
        } else {
            eprintln!("[DEBUG] Bot {} not in running_bots map, skipping cycle", bot_id);
            return Ok(());
        };
        let bot = queries::get_bot_by_id(&self.db, bot_id, user_id).await.map_err(|e| e.to_string())?.ok_or("Bot not found")?;
        let portfolio = queries::get_portfolio(&self.db, bot_id, user_id).await.map_err(|e| e.to_string())?.ok_or("No portfolio")?;

        // --- LIVE BALANCE SYNC & SECURITY CHECK ---
        if bot.trading_mode == "live" {
            let mut has_creds = false;
            if let Some(ref cache) = credential_cache {
                let c = cache.read().await;
                if let Some(creds) = c.get(&user_id) {
                    has_creds = true;
                    // Megpróbáljuk lekérni a valódi egyenleget a tőzsdéről
                    match Self::fetch_live_balance(creds).await {
                        Ok(real_balance) => {
                            if (real_balance - portfolio.balance).abs() > 0.001 {
                                tracing::info!("[LIVE] Syncing real balance for bot {}: ${:.2}", bot_id, real_balance);
                                queries::update_portfolio_balance(&self.db, bot_id, real_balance).await.ok();
                                self.event_sender.send(BotEvent::BalanceUpdated { bot_id, balance: real_balance }).ok();
                            }
                        }
                        Err(e) => {
                            tracing::error!("[LIVE] Failed to fetch balance for bot {}: {}", bot_id, e);
                        }
                    }
                }
            }

            if !has_creds {
                // NINCS KULCS -> Biztonsági okokból 0-ra állítjuk a UI-t és megállunk
                if portfolio.balance > 0.0 {
                    queries::update_portfolio_balance(&self.db, bot_id, 0.0).await.ok();
                    self.event_sender.send(BotEvent::BalanceUpdated { bot_id, balance: 0.0 }).ok();
                }
                
                // Küldünk egy hibaüzenetet a frontend naplóba is
                self.event_sender.send(BotEvent::Error { 
                    bot_id, 
                    message: "ÉLES MÓD: Hiányoznak az API kulcsok a Beállításokban!".into() 
                }).ok();
                
                return Err("Live mode requires API credentials. Please set them in Settings.".into());
            }
        }

        let fresh_balance_for_stop = queries::get_portfolio(&self.db, bot_id, user_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.balance)
            .unwrap_or(portfolio.balance);
        // --- DYNAMIC STOP LOSS ---
        let stop_loss_pct = bot.stop_loss.abs().max(10.0); // Minimum 10% védelem
        let stop_threshold = portfolio.initial_balance * (1.0 - stop_loss_pct / 100.0);

        if fresh_balance_for_stop <= stop_threshold {
            tracing::warn!("Bot {} hit stop-loss: balance={:.2}, threshold={:.2}", bot_id, fresh_balance_for_stop, stop_threshold);
            
            // Telegram értesítés stop-lossról
            if let Some(ref telegram) = self.telegram_service {
                let t = telegram.clone();
                let b_name = bot.name.clone();
                tokio::spawn(async move {
                    let msg = format!("⚠️ <b>Bot STOP-LOSS Hit!</b>\n\nBot: <b>{}</b>\nEgyenleg: <b>${:.2}</b>\nBot leállítva a tőke védelmében.", b_name, fresh_balance_for_stop);
                    let _ = t.send_message(user_id, &msg).await;
                });
            }

            self.stop_bot(bot_id, user_id).await?;
            return Ok(());
        }

        let all_markets = fetch_active_markets("5").await;
        // FIX: Minden bot különböző piacot kap, bot_id alapján round-robin elosztással.
        // Ha nincs elég piac, az első piacon osztoznak.
        let market = if all_markets.is_empty() {
            return Ok(());
        } else {
            let idx = (bot_id as usize) % all_markets.len();
            all_markets[idx].clone()
        };
        let btc_price = self.fetch_btc_price().await?;
        
        // Calculate BTC change and velocity/acceleration from price history
        let btc_change;
        let btc_velocity;
        let btc_acceleration;
        {
            let now = Instant::now();
            rb.btc_price_history.push((btc_price, now));
            let cutoff = now - Duration::from_secs(60);
            rb.btc_price_history.retain(|(_, t)| *t > cutoff);
            
            if rb.btc_price_history.len() >= 2 {
                let oldest_price = rb.btc_price_history.first().map(|(p, _)| *p).unwrap_or(btc_price);
                btc_change = Some((btc_price - oldest_price) / oldest_price);
                
                let first_instant = rb.btc_price_history.first().map(|(_, t)| *t).unwrap();
                let last_instant = rb.btc_price_history.last().map(|(_, t)| *t).unwrap();
                let duration_secs = (last_instant - first_instant).as_secs_f64().max(1.0);
                
                btc_velocity = Some(btc_change.unwrap() / duration_secs);
                
                if rb.btc_price_history.len() >= 3 {
                    let mid_idx = rb.btc_price_history.len() / 2;
                    let mid_price = rb.btc_price_history[mid_idx].0;
                    let mid_instant = rb.btc_price_history[mid_idx].1;
                    
                    let first_to_mid_secs = (mid_instant - first_instant).as_secs_f64().max(1.0);
                    let mid_to_last_secs = (last_instant - mid_instant).as_secs_f64().max(1.0);
                    
                    let velocity_first_half = (mid_price - oldest_price) / oldest_price / first_to_mid_secs;
                    let velocity_second_half = (btc_price - mid_price) / mid_price / mid_to_last_secs;
                    
                    btc_acceleration = Some((velocity_second_half - velocity_first_half) / (duration_secs / 2.0).max(1.0));
                } else {
                    btc_acceleration = Some(0.0);
                }
            } else {
                btc_change = rb.last_btc_price.map(|last| (btc_price - last) / last);
                btc_velocity = btc_change;
                btc_acceleration = Some(0.0);
            }
        }
        
        let market_slug = market.condition_id.clone();
        self.event_sender.send(BotEvent::Scanning { bot_id, market_slug: market_slug.clone() }).ok();

        let market_ended = market.time_remaining <= 5;
        let market_changed = rb.last_market_slug.as_ref() != Some(&market_slug);
        
        if market_changed || market_ended {
            if let Some(ref bet) = rb.pending_bet {
                let diff = (btc_price - bet.start_price) / bet.start_price;

                // Helyes Polymarket settlement logika:
                // Ha van price_to_beat: YES nyer ha final BTC >= price_to_beat
                // Ha nincs: fallback a BTC irány alapján
                let won = if let Some(ptb) = bet.price_to_beat {
                    if bet.side == "YES" { btc_price >= ptb } else { btc_price < ptb }
                } else {
                    tracing::error!("[SETTLEMENT] Missing price_to_beat for bot {}. Falling back to start_price. This is mathematically inaccurate!", bot_id);
                    let min_diff = 0.0001_f64;
                    if bet.side == "YES" { diff > min_diff } else { diff < -min_diff }
                };
                
                let effective_cost = if bet.side == "YES" { bet.entry_price } else { 1.0 - bet.entry_price };
                let profit_if_won = bet.bet_size * (1.0 - effective_cost);
                let polymarket_fee_rate = 0.02;
                let settlement_credit = if won {
                    // Collateral returned + (profit minus 2% Polymarket fee)
                    bet.bet_size * effective_cost + profit_if_won * (1.0 - polymarket_fee_rate)
                } else {
                    0.0
                };
                
                let pnl_for_stats = if won { 
                    profit_if_won * (1.0 - polymarket_fee_rate) 
                } else { 
                    -(bet.bet_size * effective_cost) 
                };
                
                queries::record_paper_settlement(&self.db, bot_id, bet.decision_id, won, settlement_credit, pnl_for_stats).await.ok();
                
                // Telegram értesítés
                if let Some(ref telegram) = self.telegram_service {
                    let icon = if won { "✅" } else { "❌" };
                    let status_text = if won { "NYERT" } else { "VESZTETT" };
                    let msg = format!(
                        "{} <b>Bot Trade Result</b>\n\nBot: <b>{}</b>\nEredmény: <b>{}</b>\nPnL: <b>${:.2}</b>\nBTC: ${:.2}",
                        icon, bot.name, status_text, pnl_for_stats, btc_price
                    );
                    let t = telegram.clone();
                    tokio::spawn(async move {
                        let _ = t.send_message(user_id, &msg).await;
                    });
                }

                self.event_sender.send(BotEvent::TradeResult { bot_id, won, pnl: pnl_for_stats }).ok();
                eprintln!("[SETTLE] Bot {}: {} won={} credit={:.4} pnl={:.4} price_diff={:.6}",
                    bot_id, bet.side, won, settlement_credit, pnl_for_stats, diff);

                {
                    let mut rm = self.risk_manager.write().await;
                    rm.record_trade_result(bot_id, won);
                }

                rb.pending_bet = None;
            }
            
            if market_changed {
                rb.btc_window_open = Some(btc_price);
                rb.last_market_slug = Some(market_slug.clone());
                tracing::info!("[MARKET] Bot {} new market: {} (time_remaining={}s)", bot_id, market_slug, market.time_remaining);
            }
        }

        let ctx = StrategyContext {
            btc_price,
            btc_change,
            btc_window_open: rb.btc_window_open,
            yes_price: market.yes_price,
            no_price: market.no_price,
            time_remaining: market.time_remaining,
            btc_velocity: btc_velocity,
            btc_acceleration: btc_acceleration,
            btc_volatility: if rb.btc_price_history.len() >= 3 {
                let prices: Vec<f64> = rb.btc_price_history.iter().map(|(p, _)| *p).collect();
                let returns: Vec<f64> = prices.windows(2).map(|w| (w[1] - w[0]) / w[0]).collect();
                let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
                Some(variance.sqrt())
            } else {
                None
            },
        };

        let signal = rb.strategy.evaluate_with_context(ctx);
        eprintln!("[SIGNAL] Bot {} signal: {:?}", bot_id, signal);

        // MINDEN szignált (a HOLD-ot is) elküldünk az SSE-n, hogy látszódjon a webes konzolban
        match &signal {
            Signal::Hold(reason) => {
                self.event_sender.send(BotEvent::TradeDecision { 
                    bot_id, 
                    bot_name: rb.bot_name.clone(),
                    outcome: "HOLD".to_string(), 
                    confidence: 0.0, 
                    bet_size: 0.0, 
                    reason: reason.clone() 
                }).ok();
            },
            Signal::Yes(conf) | Signal::No(conf) => {
                let outcome = if matches!(signal, Signal::Yes(_)) { "YES" } else { "NO" };
                
                if rb.pending_bet.is_none() {
                    let price = market.yes_price;
                    let effective_cost = if outcome == "YES" { price } else { 1.0 - price };

                    // EV check
                    let polymarket_fee_rate = 0.02;
                    let expected_value_positive = *conf > (effective_cost + polymarket_fee_rate);

                    let (can_trade, block_reason) = if !expected_value_positive {
                        (false, Some(format!("Negative EV (conf {:.2} <= cost {:.2} + fee {:.2})", conf, effective_cost, polymarket_fee_rate)))
                    } else {
                        let mut rm = self.risk_manager.write().await;
                        rm.can_open_position(
                            bot_id,
                            bot.bet_size,
                            *conf,
                            fresh_balance_for_stop,
                            portfolio.initial_balance,
                        )
                    };

                    if !can_trade {
                        let reason = block_reason.unwrap_or_else(|| "Risk blocked".into());
                        tracing::info!("[RISK] Bot {} blocked: {}", bot_id, reason);
                        eprintln!("[RISK] Bot {} blocked: {}", bot_id, reason);
                        
                        // SSE-n is elküldjük, hogy miért lett blokkolva a belépés
                        self.event_sender.send(BotEvent::TradeDecision { 
                            bot_id, 
                            bot_name: rb.bot_name.clone(),
                            outcome: "HOLD".to_string(), 
                            confidence: *conf, 
                            bet_size: 0.0, 
                            reason: format!("RISK BLOCKED: {}", reason) 
                        }).ok();
                    } else {
                        self.event_sender.send(BotEvent::TradeDecision { 
                            bot_id, 
                            bot_name: rb.bot_name.clone(),
                            outcome: outcome.to_string(), 
                            confidence: *conf, 
                            bet_size: bot.bet_size, 
                            reason: "Signal detected & Risk approved".into() 
                        }).ok();
                        
                        if bot.trading_mode == "live" {
                            if let Some(ref cache) = credential_cache {
                                let c = cache.read().await;
                                if let Some(creds) = c.get(&user_id) {
                                    let _ = Self::place_order(&market, outcome, bot.bet_size, creds).await;
                                }
                            }
                        } else {
                            let d_id = queries::log_trade_decision(&self.db, bot_id, rb.session_id, user_id, &market_slug, &market.condition_id, outcome, &bot.strategy_type, *conf, Some(btc_price), btc_change, Some(market.yes_price), Some(market.no_price), Some(market.time_remaining), "paper trade").await.unwrap_or(0);
                            
                            let fresh_balance = queries::get_portfolio(&self.db, bot_id, user_id)
                                .await
                                .ok()
                                .flatten()
                                .map(|p| p.balance)
                                .unwrap_or(portfolio.balance);
                            
                            queries::update_portfolio_balance(&self.db, bot_id, fresh_balance - bot.bet_size * effective_cost).await.ok();
                            eprintln!("[BET] Bot {}: {} ${:.2} @ {:.2} | balance: {:.2} → {:.2}",
                                bot_id, outcome, bot.bet_size, price, fresh_balance, fresh_balance - bot.bet_size * effective_cost);
                            
                            rb.pending_bet = Some(PendingBet {
                                side: outcome.to_string(),
                                bet_size: bot.bet_size,
                                start_price: btc_price,
                                entry_price: price,
                                decision_id: d_id,
                                price_to_beat: market.price_to_beat,
                                market_end_time: market.end_time,
                            });
                            rb.last_trade_time = Some(Instant::now());
                            
                            // Telegram értesítés trade indításról
                            if let Some(ref telegram) = self.telegram_service {
                                let msg = format!(
                                    "🚀 <b>Bot Trade Opened</b>\n\nBot: <b>{}</b>\nIrány: <b>{}</b>\nTét: <b>${:.2}</b>\nBTC: ${:.2}",
                                    bot.name, outcome, bot.bet_size, btc_price
                                );
                                let t = telegram.clone();
                                tokio::spawn(async move {
                                    let _ = t.send_message(user_id, &msg).await;
                                });
                            }

                            self.event_sender.send(BotEvent::PositionUpdate { bot_id, side: outcome.to_string(), size: bot.bet_size, price, unrealized_pnl: 0.0 }).ok();
                        }
                    }
                }
            }
        }

        rb.last_btc_price = Some(btc_price);
        
        {
            let mut running = self.running_bots.write().await;
            if let Some(existing) = running.get_mut(&bot_id) {
                // Update only the fields that changed during the cycle
                existing.last_btc_price = rb.last_btc_price;
                existing.btc_window_open = rb.btc_window_open;
                existing.last_market_slug = rb.last_market_slug;
                existing.pending_bet = rb.pending_bet;
                existing.btc_price_history = rb.btc_price_history;
                existing.last_trade_time = rb.last_trade_time;
                // Note: current_balance is usually updated via DB queries/events, 
                // so we don't overwrite it here to avoid race conditions with balance updates.
            }
        }
        Ok(())
    }

    async fn fetch_btc_price(&self) -> Result<f64, String> {
        let resp = reqwest::get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT").await.map_err(|e| e.to_string())?;
        #[derive(serde::Deserialize)] struct BP { price: String }
        let data: BP = resp.json().await.map_err(|e| e.to_string())?;
        data.price.parse::<f64>().map_err(|e| e.to_string())
    }

    async fn place_order(market: &crate::api::market::ActiveMarket, outcome: &str, bet_size: f64, creds: &CachedCredentials) -> Result<String, String> {
        // Increased slippage buffer to 0.8% for even better fill reliability
        let slippage = 1.008; 
        let order_price = if outcome == "YES" { (market.yes_price * slippage).min(0.99) } else { ((1.0 - market.yes_price) * slippage).min(0.99) };

        // Create PolymarketClient from credentials
        let client = match PolymarketClient::from_api_credentials(
            &creds.private_key,
            creds.signature_type,
            Some(crate::trading::polymarket::ApiKeyCreds {
                key: creds.api_key.clone(),
                secret: creds.api_secret.clone(),
                passphrase: creds.api_passphrase.clone(),
            }),
            creds.funder.as_deref(),
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to create PolymarketClient for live order: {}", e);
                return Err(format!("Failed to create client: {}", e));
            }
        };

        let token_id = if outcome == "YES" { market.yes_token_id.clone() } else { market.no_token_id.clone() };
        let side = if outcome == "YES" { "BUY" } else { "BUY" }; // Mindig BUY, ha kimenetelre fogadunk

        // Sign and post the order using V2 and passing the neg_risk flag from the market
        match client.create_order_v2(&OrderRequest {
            token_id,
            price: order_price,
            size: bet_size / order_price, // bet_size is USD, size is tokens
            side: side.to_string(),
        }, market.neg_risk).await {
            Ok(signed_order) => {
                match client.post_order(&signed_order).await {
                    Ok(response) => {
                        let order_id = response.order_id.unwrap_or_else(|| "unknown".to_string());
                        tracing::info!("Live order placed: id={}, outcome={}, size={}, price={}", order_id, outcome, bet_size, order_price);
                        Ok(order_id)
                    }
                    Err(e) => {
                        tracing::error!("Failed to post order: {}", e);
                        Err(format!("Failed to post order: {}", e))
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to create order: {}", e);
                Err(format!("Failed to create order: {}", e))
            }
        }
    }

    pub async fn is_running(&self, bot_id: i64) -> bool {
        self.running_bots.read().await.contains_key(&bot_id)
    }

    pub async fn get_all_running_bots(&self) -> Vec<i64> {
        self.running_bots.read().await.keys().copied().collect()
    }

    pub async fn get_running_bots(&self, user_id: i64) -> Vec<i64> {
        self.running_bots
            .read()
            .await
            .iter()
            .filter(|(_, b)| b.user_id == user_id)
            .map(|(id, _)| *id)
            .collect()
    }

    pub async fn auto_save_sessions(&self) -> Result<(), String> {
        let running = self.running_bots.read().await;
        let bot_count = running.len();
        let pending_count = running.values().filter(|rb| rb.pending_bet.is_some()).count();
        if pending_count > 0 {
            tracing::debug!("Auto-save: {} running bots, {} with pending bets", bot_count, pending_count);
        }
        drop(running);
        Ok(())
    }

    pub async fn has_pending_bet(&self, bot_id: i64) -> bool {
        self.running_bots
            .read()
            .await
            .get(&bot_id)
            .map(|rb| rb.pending_bet.is_some())
            .unwrap_or(false)
    }

    pub async fn get_pending_bet_size(&self, bot_id: i64) -> f64 {
        self.running_bots
            .read()
            .await
            .get(&bot_id)
            .and_then(|rb| rb.pending_bet.as_ref())
            .map(|bet| bet.bet_size)
            .unwrap_or(0.0)
    }

    async fn fetch_live_balance(creds: &CachedCredentials) -> Result<f64, String> {
        use super::polymarket::{PolymarketClient, ApiKeyCreds};
        let client = PolymarketClient::new(&creds.private_key)
            .map_err(|e| e.to_string())?
            .with_creds(ApiKeyCreds {
                key: creds.api_key.clone(),
                secret: creds.api_secret.clone(),
                passphrase: creds.api_passphrase.clone(),
            });

        client.get_balance().await.map_err(|e| e.to_string())
    }
}

pub async fn start_orchestrator_loop(
    orchestrator: Arc<BotOrchestrator>,
    bot_id: i64,
    user_id: i64,
    interval_secs: u64,
    credential_cache: Option<Arc<RwLock<HashMap<i64, CachedCredentials>>>>,
) {
    let mut timer = interval(Duration::from_secs(interval_secs));
    loop {
        timer.tick().await;
        if !orchestrator.is_running(bot_id).await {
            break;
        }
        let _ = orchestrator.execute_cycle(bot_id, user_id, credential_cache.clone()).await;
    }
}

pub async fn start_auto_save_loop(orchestrator: Arc<BotOrchestrator>) {
    let mut timer = interval(Duration::from_secs(30));
    loop {
        timer.tick().await;
        let _ = orchestrator.auto_save_sessions().await;
    }
}