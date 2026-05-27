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
use crate::trading::bot_executor::strategies::{Signal, StrategyExecutor, StrategyContext, MarketSnapshot};
use crate::trading::risk_manager::RiskManager;
use crate::trading::bot_loss_tracker::BotLossTrackerManager;
use crate::trading::confidence;
use crate::trading::strategy_coordinator::StrategyCoordinator;
use crate::api::market::fetch_active_markets;
use crate::api::CachedCredentials;
use crate::trading::polymarket::{PolymarketClient, OrderRequest};

#[derive(Debug, Clone, serde::Serialize)]
pub enum BotEvent {
    SessionStarted { bot_id: i64, session_id: i64, bot_name: String },
    SessionEnded { bot_id: i64, session_id: i64, final_balance: f64, total_pnl: f64 },
    TradeDecision { bot_id: i64, bot_name: String, outcome: String, confidence: f64, bet_size: f64, reason: String, asset: String, risk_multiplier: f64, kelly_bet: f64, adjusted_confidence: f64, consecutive_losses: u32 },
    OrderExecuted { bot_id: i64, order_id: String },
    BalanceUpdated { bot_id: i64, balance: f64 },
    MarketTransition { new_market_slug: String },
    Error { bot_id: i64, message: String },
    Scanning { bot_id: i64, market_slug: String, asset: String },
    Evaluating { bot_id: i64, strategy: String, confidence: f64 },
    PositionUpdate { bot_id: i64, side: String, size: f64, price: f64, unrealized_pnl: f64 },
    TradeResult { bot_id: i64, won: bool, pnl: f64 },
}

#[derive(Debug, Clone)]
pub struct PendingBet {
    pub side: String,
    pub asset: String,      // BTC, ETH, SOL, XRP
    pub bet_size: f64,
    pub start_price: f64,   // Ár fogadás nyitásakor
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
    pub strategy_type: String,
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
            strategy_type: bot.strategy_type.clone(),
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
            strategy_type: bot.strategy_type.clone(),
        });
        // Fix: tell RiskManager the actual starting balance so portfolio loss limits are correct
        {
            let mut rm = self.risk_manager.write().await;
            rm.set_portfolio_start_balance(current_balance);
        }
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
            strategy_type: bot.strategy_type.clone(),
        });
        drop(running);
        // Fix: tell RiskManager the actual starting balance so portfolio loss limits are correct
        {
            let mut rm = self.risk_manager.write().await;
            rm.set_portfolio_start_balance(initial_balance);
        }
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

        // === EARLY SETTLEMENT: process pending bets even when no markets available ===
        if all_markets.is_empty() {
            if let Some(ref bet) = rb.pending_bet {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                if now > bet.market_end_time {
                    tracing::info!("[SETTLE] Bot {}: early settlement triggered (no active markets)", bot_id);
                    let current_settle_price = crate::api::market::get_asset_price(&bet.asset).await.unwrap_or(0.0);
                    if current_settle_price > 0.0 {
                        let diff = (current_settle_price - bet.start_price) / bet.start_price;
                        let won = if let Some(ptb) = bet.price_to_beat {
                            let is_won = if bet.side == "YES" { current_settle_price >= ptb } else { current_settle_price < ptb };
                            tracing::info!("[SETTLE-EMPTY] Bot {}: {} Target={:.2} Final={:.2} Won={}", bot_id, bet.side, ptb, current_settle_price, is_won);
                            is_won
                        } else {
                            let moved_up = current_settle_price > bet.start_price;
                            let moved_same_direction = (bet.side == "YES" && moved_up) || (bet.side == "NO" && !moved_up);
                            tracing::warn!("[SETTLE-EMPTY] Missing price_to_beat for bot {}. Using direction: start={:.2} → final={:.2}, same={}",
                                bot_id, bet.start_price, current_settle_price, moved_same_direction);
                            moved_same_direction
                        };

                        let polymarket_fee_rate = 0.00;
                        let potential_return = bet.bet_size / bet.entry_price.max(0.01);
                        let settlement_credit = if won { potential_return * (1.0 - polymarket_fee_rate) } else { 0.0 };
                        let pnl_for_stats = settlement_credit - bet.bet_size;

                        queries::record_paper_settlement(&self.db, bot_id, bet.decision_id, won, settlement_credit, pnl_for_stats).await.ok();

                        self.event_sender.send(BotEvent::TradeResult { bot_id, won, pnl: pnl_for_stats }).ok();
                        eprintln!("[SETTLE-EMPTY] Bot {}: {} won={} credit={:.4} pnl={:.4} price_diff={:.6}",
                            bot_id, bet.side, won, settlement_credit, pnl_for_stats, diff);

                        {
                            let mut rm = self.risk_manager.write().await;
                            rm.record_trade_result(bot_id, won);
                        }

                        {
                            let mut lt = self.loss_tracker.write().await;
                            lt.update_settlement(bot_id, won, pnl_for_stats, fresh_balance_for_stop);
                        }

                        rb.pending_bet = None;
                    }
                }
            }

            // Update running bot state before returning
            {
                let mut running = self.running_bots.write().await;
                if let Some(existing) = running.get_mut(&bot_id) {
                    existing.pending_bet = rb.pending_bet;
                }
            }
            return Ok(());
        }

        // Kriptovaluta kiválasztás a bot beállítása alapján
        let market = {
            let _target_asset = bot.market_id.to_uppercase();
            let target_asset = bot.market_id.to_uppercase();
            if target_asset != "AUTO" {
                // Kifejezetten ezt az eszközt kérték (pl. BTC, ETH)
                if let Some(m) = all_markets.iter().find(|m| m.asset == target_asset) {
                    m.clone()
                } else {
                    // Ha nincs ilyen aktív piac, akkor várunk a következő ciklusra
                    return Ok(());
                }
            } else {
                // AUTO mód: Round-robin elosztás
                let idx = (bot_id as usize) % all_markets.len();
                all_markets[idx].clone()
            }
        };

        // A piacnak megfelelő eszköz árfolyamát kérjük le
        let asset_price = crate::api::market::get_asset_price(&market.asset).await.unwrap_or(0.0);
        if asset_price == 0.0 {
            return Ok(());
        }
        
        // Calculate asset change and velocity/acceleration from price history
        let asset_change;
        let asset_velocity;
        let asset_acceleration;
        {
            let now = Instant::now();
            rb.btc_price_history.push((asset_price, now));
            // Keep only last 30 seconds of history for velocity calc
            let cutoff = now - Duration::from_secs(30);
            rb.btc_price_history.retain(|(_, t)| *t > cutoff);

            if rb.btc_price_history.len() >= 2 {
                // Change from oldest in window
                let oldest = rb.btc_price_history.first().map(|(p, _)| *p).unwrap_or(asset_price);
                asset_change = Some((asset_price - oldest) / oldest);

                // Velocity: % change per second over window (duration_secs evaluates to 1.0 based on reference repo)
                let duration_secs = rb.btc_price_history.last().map(|(_, t)| t.elapsed().as_secs_f64()).unwrap_or(1.0).max(1.0);
                asset_velocity = Some(asset_change.unwrap() / duration_secs);

                // Acceleration: change in velocity (simplified)
                if rb.btc_price_history.len() >= 3 {
                    let oldest2 = rb.btc_price_history[rb.btc_price_history.len()/2].0;
                    let mid_change = (oldest2 - oldest) / oldest;
                    let mid_duration = duration_secs / 2.0;
                    let prev_velocity = mid_change / mid_duration.max(1.0);
                    asset_acceleration = Some((asset_velocity.unwrap() - prev_velocity) / mid_duration.max(1.0));
                } else {
                    asset_acceleration = Some(0.0);
                }
            } else {
                // Fallback to last_btc_price (velocity = change / 5sec interval assumption)
                asset_change = rb.last_btc_price.map(|last| (asset_price - last) / last);
                asset_velocity = asset_change.map(|c| c / 5.0);
                asset_acceleration = Some(0.0);
            }
        }
        
        let market_slug = market.condition_id.clone();
        self.event_sender.send(BotEvent::Scanning { bot_id, market_slug: market_slug.clone(), asset: market.asset.clone() }).ok();

        // Ha még nincs window_open beállítva (pl. restore utáni első ciklus,
        // vagy amikor korábban nem volt elérhető piac), állítsuk be most
        if rb.btc_window_open.is_none() {
            rb.btc_window_open = Some(asset_price);
            eprintln!("[MARKET] Bot {} initial window_open set to {:.2}", bot_id, asset_price);
        }

        let market_ended = market.time_remaining <= 5;
        let market_changed = rb.last_market_slug.as_ref() != Some(&market_slug);
        
        if market_changed || market_ended {
            if market_ended {
                let mut coord = self.coordinator.write().await;
                coord.reset_market(&market_slug);
            }
            if let Some(ref bet) = rb.pending_bet {
                // Elszámoláshoz a fogadáskori eszköz árát kérjük le
                let current_settle_price = crate::api::market::get_asset_price(&bet.asset).await.unwrap_or(asset_price);
                let diff = (current_settle_price - bet.start_price) / bet.start_price;

                let won = if let Some(ptb) = bet.price_to_beat {
                    // 1st choice: stored price_to_beat from when the bet was placed
                    let is_won = if bet.side == "YES" { current_settle_price >= ptb } else { current_settle_price < ptb };
                    tracing::info!("[SETTLE] Bot {}: {} Target={:.2} Final={:.2} Won={}", bot_id, bet.side, ptb, current_settle_price, is_won);
                    is_won
                } else if let Some(market_ptb) = market.price_to_beat {
                    // 2nd choice: fresh price_to_beat from the active market
                    let is_won = if bet.side == "YES" { current_settle_price >= market_ptb } else { current_settle_price < market_ptb };
                    tracing::info!("[SETTLE] Bot {}: {} fresh_market_ptb={:.2} Final={:.2} Won={}", bot_id, bet.side, market_ptb, current_settle_price, is_won);
                    is_won
                } else {
                    // 3rd choice: fallback — check if price moved in our direction
                    // For a 5-min Polymarket BTC market, if we can't get the exact price_to_beat,
                    // we compare the direction: if price went up and we bet YES, that's likely a win.
                    let moved_up = current_settle_price > bet.start_price;
                    let moved_same_direction = (bet.side == "YES" && moved_up) || (bet.side == "NO" && !moved_up);
                    tracing::warn!("[SETTLEMENT] Missing price_to_beat for bot {}. Using direction fallback: start={:.2} → final={:.2}, moved_same_direction={}",
                        bot_id, bet.start_price, current_settle_price, moved_same_direction);
                    moved_same_direction
                };
                
                let polymarket_fee_rate = 0.00;
                let potential_return = bet.bet_size / bet.entry_price.max(0.01);
                let settlement_credit = if won {
                    potential_return * (1.0 - polymarket_fee_rate)
                } else {
                    0.0
                };
                
                let pnl_for_stats = settlement_credit - bet.bet_size;
                
                queries::record_paper_settlement(&self.db, bot_id, bet.decision_id, won, settlement_credit, pnl_for_stats).await.ok();
                
                // Telegram értesítés
                if let Some(ref telegram) = self.telegram_service {
                    let icon = if won { "✅" } else { "❌" };
                    let status_text = if won { "NYERT" } else { "VESZTETT" };
                    let msg = format!(
                        "{} <b>Bot Trade Result</b>\n\nBot: <b>{}</b>\nEredmény: <b>{}</b>\nPnL: <b>${:.2}</b>\nÁr: ${:.2} ({})",
                        icon, bot.name, status_text, pnl_for_stats, current_settle_price, bet.asset
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

                // Update the loss tracker so that adjust_confidence() and get_risk_multiplier()
                // have up-to-date consecutive loss/win data
                {
                    let mut lt = self.loss_tracker.write().await;
                    // fresh_balance_for_stop already reflects the balance AFTER record_paper_settlement was called
                    // DO NOT add pnl_for_stats again (would double-count PnL in peak_balance/drawdown tracking)
                    lt.update_settlement(bot_id, won, pnl_for_stats, fresh_balance_for_stop);
                }

                rb.pending_bet = None;
            }
            
            if market_changed {
                if let Some(ref old_slug) = rb.last_market_slug {
                    let mut coord = self.coordinator.write().await;
                    coord.reset_market(old_slug);
                }
                rb.btc_window_open = Some(asset_price);
                rb.last_market_slug = Some(market_slug.clone());
                tracing::info!("[MARKET] Bot {} new market: {} (time_remaining={}s)", bot_id, market_slug, market.time_remaining);
            }
        }

        let mut snapshot = MarketSnapshot::new(market_slug.clone());
        snapshot.question = market.question.clone();
        snapshot.yes_price = market.yes_price;
        snapshot.no_price = market.no_price;
        snapshot.time_remaining = market.time_remaining;
        snapshot.btc_price = asset_price;
        snapshot.btc_change_24h = asset_change;
        snapshot.btc_velocity = asset_velocity;
        snapshot.btc_acceleration = asset_acceleration;
        snapshot.btc_window_open = rb.btc_window_open;
        snapshot.market_start_price = Some(market.price_to_beat.or(rb.pending_bet.as_ref().map(|b| b.start_price)).unwrap_or(asset_price));

        let ctx = StrategyContext {
            btc_price: asset_price,
            btc_change: asset_change,
            btc_window_open: rb.btc_window_open,
            yes_price: market.yes_price,
            no_price: market.no_price,
            time_remaining: market.time_remaining,
            btc_velocity: asset_velocity,
            btc_acceleration: asset_acceleration,
            market_start_price: snapshot.market_start_price,
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

        // Dynamically adjust min_delta based on the market asset being scanned
        let mut strategy = rb.strategy.clone();
        let asset_lower = market.asset.to_lowercase();
        match asset_lower.as_str() {
            "btc" => strategy.params.min_delta = 0.0006, // ~0.06% (volt: 0.3%) - érzékeny korai trend-belépés
            "eth" => strategy.params.min_delta = 0.0010, // ~0.10% (volt: 0.5%)
            "sol" => strategy.params.min_delta = 0.0018, // ~0.18% (volt: 0.8%)
            "xrp" => strategy.params.min_delta = 0.0012, // ~0.12% (volt: 0.6%)
            _ => strategy.params.min_delta = 0.0010,
        }
        tracing::debug!(
            "Bot {} ({}) scanning {}; overriding min_delta to {:.4}",
            bot_id,
            rb.bot_name,
            market.asset,
            strategy.params.min_delta
        );

        let signal = strategy.evaluate_with_context(ctx.clone());
        eprintln!("[SIGNAL] Bot {} signal: {:?}", bot_id, signal);

        // MINDEN szignált (a HOLD-ot is) elküldünk az SSE-n, hogy látszódjon a webes konzolban
        match &signal {
            Signal::Hold(reason) => {
                // No risk metrics computed for Hold (no trade attempted)
                let lt = self.loss_tracker.read().await;
                let consec_losses = lt.get_consecutive_losses(bot_id);
                drop(lt);
                self.event_sender.send(BotEvent::TradeDecision { 
                    bot_id, 
                    bot_name: rb.bot_name.clone(),
                    outcome: "HOLD".to_string(), 
                    confidence: 0.0, 
                    bet_size: 0.0, 
                    reason: reason.clone(),
                    asset: market.asset.clone(),
                    risk_multiplier: 1.0,
                    kelly_bet: 0.0,
                    adjusted_confidence: 0.0,
                    consecutive_losses: consec_losses,
                }).ok();
            },
            Signal::Yes(conf) | Signal::No(conf) => {
                let outcome = if matches!(signal, Signal::Yes(_)) { "YES" } else { "NO" };
                
                if rb.pending_bet.is_none() {
                    let slippage_sim = 1.005;
                    let price = if outcome == "YES" { (market.yes_price * slippage_sim).min(0.99) } else { (market.no_price * slippage_sim).min(0.99) };
                    let effective_cost = price;

                    let polymarket_fee_rate = 0.00;
                    let min_conf_threshold = 0.42;

                    // === INTEGRATED CONFIDENCE + EV + KELLY PIPELINE ===
                    // Ported from polymarket-demo TypeScript: calculate7FactorConfidence(), calculateEV(), calculateBetSize()

                    // Step 1: Adjust confidence using loss tracker (performance feedback)
                    let seven_factor_conf = confidence::calculate_7_factor_confidence(&ctx, outcome);
                    let blended_conf = (*conf * 0.4 + seven_factor_conf * 0.6).clamp(0.05, 0.95);
                    let adjusted_conf = {
                        let mut lt = self.loss_tracker.write().await;
                        lt.adjust_confidence(bot_id, blended_conf, fresh_balance_for_stop)
                    };

                    // Step 2: Calculate BTC signal strength for Bayesian update
                    let btc_signal_strength = asset_change
                        .map(|c| (c.abs() * 500.0).clamp(0.0, 1.0))
                        .unwrap_or(0.0);

                    // Step 3: Bayesian EV with proper formula
                    // EV = P(win) × (1 - cost) × (1 - fee) - P(lose) × cost
                    let (bayes_prob, expected_value) = confidence::calculate_bayesian_ev(
                        adjusted_conf,
                        effective_cost,
                        polymarket_fee_rate,
                        btc_signal_strength,
                    );

                    // Step 4: Get risk multiplier from loss tracker (consecutive loss / drawdown)
                    let (risk_mult, consecutive_losses) = {
                        let mut lt = self.loss_tracker.write().await;
                        let rm = lt.get_risk_multiplier(bot_id, fresh_balance_for_stop);
                        let info = lt.get_tracker_info(bot_id, fresh_balance_for_stop);
                        (rm, info.consecutive_losses)
                    };

                    // Step 5: Calculate half-Kelly bet size with risk multiplier
                    let kelly_fraction = bot.kelly_fraction.max(0.1);
                    let max_bet_fraction = 0.25; // Max 25% of bankroll per trade
                    let min_bet = 0.10; // Fixed minimum bet ($0.10)
                    let max_bet = bot.bet_size.max(min_bet); // bet_size = maximum cap (user-facing setting)

                    let kelly_bet = confidence::calculate_half_kelly_bet(
                        fresh_balance_for_stop,
                        bayes_prob,
                        effective_cost,
                        kelly_fraction,
                        risk_mult,
                        max_bet_fraction,
                        min_bet,
                    );

                    // Use Kelly bet if positive edge exists, otherwise use min_bet
                    // Then cap at max_bet (the bot's bet_size setting)
                    let mut final_bet = if kelly_bet > 0.0 { kelly_bet.max(min_bet) } else { min_bet };
                    final_bet = final_bet.min(max_bet); // Hard cap at bet_size

                    let expected_value_positive = expected_value > 0.0 && bayes_prob >= min_conf_threshold;

                    // Prevent momentum entry in last 60s (too risky)
                    let time_is_okay = market.time_remaining > 60 || rb.strategy_type == "last_seconds_scalp";

                    let mut block_reason = None;
                    let mut can_trade = false;
                    let mut coordinator_final_bet = final_bet;

                    if !expected_value_positive {
                        block_reason = Some(format!("Low EV/Conf (prob {:.2} < threshold {:.2}, EV={:.4})", bayes_prob, min_conf_threshold, expected_value));
                    } else if !time_is_okay {
                        block_reason = Some(format!("Too late to trade ({}s remaining)", market.time_remaining));
                    } else {
                        let mut rm = self.risk_manager.write().await;
                        let (rm_allowed, rm_reason) = rm.can_open_position(
                            bot_id,
                            final_bet,
                            bayes_prob,
                            fresh_balance_for_stop,
                            portfolio.initial_balance,
                        );
                        if !rm_allowed {
                            block_reason = rm_reason;
                        } else {
                            // Check StrategyCoordinator
                            let mut coord = self.coordinator.write().await;
                            let coord_res = coord.register_decision(
                                &market.condition_id,
                                bot_id,
                                &rb.bot_name,
                                &rb.strategy_type,
                                outcome,
                                bayes_prob,
                                final_bet,
                                fresh_balance_for_stop,
                            );
                            if coord_res.allowed {
                                can_trade = true;
                                if let Some(adj) = coord_res.adjusted_bet_size {
                                    coordinator_final_bet = adj;
                                }
                            } else {
                                block_reason = Some(format!("Coordinator blocked: {}", coord_res.reason));
                            }
                        }
                    }

                    let final_bet = coordinator_final_bet;

                    if !can_trade {
                        let reason = block_reason.unwrap_or_else(|| "Risk blocked".into());
                        tracing::info!("[RISK] Bot {} blocked: {} (risk_mult={:.2}, adj_conf={:.2}, kelly={:.2})",
                            bot_id, reason, risk_mult, adjusted_conf, kelly_bet);
                        eprintln!("[RISK] Bot {} blocked: {}", bot_id, reason);

                        self.event_sender.send(BotEvent::TradeDecision {
                            bot_id,
                            bot_name: rb.bot_name.clone(),
                            outcome: "HOLD".to_string(),
                            confidence: bayes_prob,
                            bet_size: 0.0,
                            reason: format!("RISK BLOCKED: {}", reason),
                            asset: market.asset.clone(),
                            risk_multiplier: risk_mult,
                            kelly_bet,
                            adjusted_confidence: adjusted_conf,
                            consecutive_losses,
                        }).ok();
                    } else {
                        self.event_sender.send(BotEvent::TradeDecision {
                            bot_id,
                            bot_name: rb.bot_name.clone(),
                            outcome: outcome.to_string(),
                            confidence: bayes_prob,
                            bet_size: final_bet,
                            reason: format!("Signal approved (risk_mult={:.2}, conf={:.2}, EV={:.4})", risk_mult, bayes_prob, expected_value),
                            asset: market.asset.clone(),
                            risk_multiplier: risk_mult,
                            kelly_bet,
                            adjusted_confidence: adjusted_conf,
                            consecutive_losses,
                        }).ok();

                        let mut order_placed = false;
                        let mut order_id = "paper_trade".to_string();

                        if bot.trading_mode == "live" {
                            if let Some(ref cache) = credential_cache {
                                let c = cache.read().await;
                                if let Some(creds) = c.get(&user_id) {
                                    match Self::place_order(&market, outcome, final_bet, creds).await {
                                        Ok(id) => {
                                            order_id = id;
                                            order_placed = true;
                                        }
                                        Err(e) => {
                                            tracing::error!("Live order failed: {}", e);
                                            // Cancel decision from coordinator since it was not placed
                                            let mut coord = self.coordinator.write().await;
                                            coord.cancel_decision(&market.condition_id, bot_id);
                                        }
                                    }
                                } else {
                                    tracing::error!("No cached credentials found for user {}", user_id);
                                    let mut coord = self.coordinator.write().await;
                                    coord.cancel_decision(&market.condition_id, bot_id);
                                }
                            } else {
                                tracing::error!("Credential cache not initialized");
                                let mut coord = self.coordinator.write().await;
                                coord.cancel_decision(&market.condition_id, bot_id);
                            }
                        } else {
                            // Paper trade is always placed
                            order_placed = true;
                        }

                        if order_placed {
                            // Confirm execution to coordinator
                            {
                                let mut coord = self.coordinator.write().await;
                                coord.confirm_execution(&market.condition_id, bot_id, outcome, final_bet);
                            }

                            let decision_reason = if bot.trading_mode == "live" { "live trade" } else { "paper trade" };
                            let d_id = queries::log_trade_decision(
                                &self.db,
                                bot_id,
                                rb.session_id,
                                user_id,
                                &market_slug,
                                &market.condition_id,
                                outcome,
                                &bot.strategy_type,
                                bayes_prob,
                                Some(asset_price),
                                asset_change,
                                Some(market.yes_price),
                                Some(market.no_price),
                                Some(market.time_remaining),
                                decision_reason
                            ).await.unwrap_or(0);

                            // Mark decision as executed in DB with order_id
                            queries::mark_decision_executed(&self.db, d_id, &order_id).await.ok();

                            let fresh_balance = queries::get_portfolio(&self.db, bot_id, user_id)
                                .await
                                .ok()
                                .flatten()
                                .map(|p| p.balance)
                                .unwrap_or(portfolio.balance);

                            queries::update_portfolio_balance(&self.db, bot_id, fresh_balance - final_bet).await.ok();
                            
                            eprintln!("[BET] Bot {}: {} ${:.2} @ {:.2} | balance: {:.2} → {:.2} (risk_mult={:.2}, conf={:.2}, mode={})",
                                bot_id, outcome, final_bet, price, fresh_balance, fresh_balance - final_bet, risk_mult, bayes_prob, bot.trading_mode);

                            // Notify loss tracker that a trade was sent (pending settlement)
                            {
                                let mut lt = self.loss_tracker.write().await;
                                lt.mark_trade_sent(bot_id);
                            }

                            rb.pending_bet = Some(PendingBet {
                                side: outcome.to_string(),
                                asset: market.asset.clone(),
                                bet_size: final_bet,
                                start_price: asset_price,
                                entry_price: price,
                                decision_id: d_id,
                                price_to_beat: market.price_to_beat,
                                market_end_time: market.end_time,
                            });
                            rb.last_trade_time = Some(Instant::now());

                            // Telegram notification
                            if let Some(ref telegram) = self.telegram_service {
                                let msg = format!(
                                    "🚀 <b>Bot Trade Opened ({})</b>\n\nBot: <b>{}</b>\nIrány: <b>{}</b>\nTét: <b>${:.2}</b>\nÁr: ${:.2} ({})\nConf: {:.1}% | RiskMult: {:.2}x",
                                    if bot.trading_mode == "live" { "LIVE" } else { "DEMO" },
                                    bot.name, outcome, final_bet, asset_price, market.asset, bayes_prob * 100.0, risk_mult
                                );
                                let t = telegram.clone();
                                tokio::spawn(async move {
                                    let _ = t.send_message(user_id, &msg).await;
                                });
                            }

                            self.event_sender.send(BotEvent::PositionUpdate {
                                bot_id,
                                side: outcome.to_string(),
                                size: final_bet,
                                price,
                                unrealized_pnl: 0.0
                            }).ok();
                        }
                    }
                }
            }
        }

        rb.last_btc_price = Some(asset_price);
        
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

    async fn place_order(market: &crate::api::market::ActiveMarket, outcome: &str, bet_size: f64, creds: &CachedCredentials) -> Result<String, String> {
        // Increased slippage buffer to 0.8% for even better fill reliability
        let slippage = 1.008; 
        let order_price = if outcome == "YES" { (market.yes_price * slippage).min(0.99) } else { (market.no_price * slippage).min(0.99) };

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