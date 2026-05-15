//! Bot Execution Engine
//!
//! Manages the continuous execution of trading strategies

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use crate::db::Db;
use crate::db::queries;
use crate::trading::PolymarketClient;
use crate::trading::bot_executor::strategies::{Signal, StrategyExecutor};
use crate::trading::market_data::MarketDataService;
use crate::trading::polymarket;

pub struct BotExecutor {
    db: Db,
    running: Arc<RwLock<bool>>,
    interval_secs: u64,
    market_service: MarketDataService,
}

impl BotExecutor {
    pub fn new(db: Db, interval_secs: u64) -> Self {
        Self {
            db,
            running: Arc::new(RwLock::new(false)),
            interval_secs,
            market_service: MarketDataService::new(),
        }
    }

    /// Start the execution loop for a specific bot
    pub async fn start_bot_loop(
        &self,
        bot_id: i64,
        user_id: i64,
        private_key: &str,
    ) -> Result<(), String> {
        let mut is_running = self.running.write().await;
        if *is_running {
            return Err("Executor already running".to_string());
        }
        *is_running = true;
        drop(is_running);

        let db = self.db.clone();
        let interval_secs = self.interval_secs;
        let running = self.running.clone();
        let market_service = self.market_service.clone();
        let private_key = private_key.to_string();

        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(interval_secs));

            loop {
                let should_run = *running.read().await;
                if !should_run {
                    tracing::info!("Bot executor loop stopped");
                    break;
                }

                interval_timer.tick().await;

                let bot = match queries::get_bot_by_id(&db, bot_id, user_id).await {
                    Ok(Some(b)) => b,
                    Ok(None) => {
                        tracing::warn!("Bot {} not found, stopping", bot_id);
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Failed to get bot config: {}", e);
                        continue;
                    }
                };

                if bot.status != "running" {
                    tracing::info!("Bot {} is not running (status: {})", bot_id, bot.status);
                    break;
                }

                if let Err(e) = Self::execute_bot_cycle(&db, bot_id, user_id, &bot, &private_key, &market_service).await {
                    tracing::error!("Bot cycle error: {}", e);
                }
            }

            *running.write().await = false;
        });

        Ok(())
    }

    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    async fn execute_bot_cycle(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        bot: &crate::db::BotRecord,
        private_key: &str,
        market_service: &MarketDataService,
    ) -> Result<(), String> {
        let snapshot = market_service.get_snapshot(&bot.market_id).await?;
        let strategy = StrategyExecutor::new(&bot.strategy_type, &bot.params);
        let signal = strategy.evaluate_with_context(snapshot.to_strategy_context());

        let signal_msg = match &signal {
            Signal::Yes(conf) => format!("BUY YES with confidence {}", conf),
            Signal::No(conf) => format!("BUY NO with confidence {}", conf),
            Signal::Hold(reason) => format!("HOLD: {}", reason),
        };

        tracing::info!("Bot {} signal: {}", bot.name, signal_msg);
        let _ = Self::log_activity(db, user_id, Some(bot_id), "INFO", &signal_msg).await;

        match signal {
            Signal::Yes(confidence) => {
                // EV check: only trade if confidence > yes_price (positive expected value)
                if confidence <= snapshot.yes_price {
                    let msg = format!("Skipping YES: confidence {:.2}% <= YES price {:.0}c (negative EV)", confidence * 100.0, snapshot.yes_price * 100.0);
                    tracing::warn!("Bot {}: {}", bot.name, msg);
                    let _ = Self::log_activity(db, user_id, Some(bot_id), "WARNING", &msg).await;
                    return Ok(());
                }
                Self::execute_trade(
                    db,
                    bot_id,
                    user_id,
                    &bot.market_id,
                    "YES",
                    confidence,
                    bot.bet_size,
                    private_key,
                ).await?;
            }
            Signal::No(confidence) => {
                // EV check: only trade if confidence > no_price (positive expected value)
                if confidence <= snapshot.no_price {
                    let msg = format!("Skipping NO: confidence {:.2}% <= NO price {:.0}c (negative EV)", confidence * 100.0, snapshot.no_price * 100.0);
                    tracing::warn!("Bot {}: {}", bot.name, msg);
                    let _ = Self::log_activity(db, user_id, Some(bot_id), "WARNING", &msg).await;
                    return Ok(());
                }
                Self::execute_trade(
                    db,
                    bot_id,
                    user_id,
                    &bot.market_id,
                    "NO",
                    confidence,
                    bot.bet_size,
                    private_key,
                ).await?;
            }
            Signal::Hold(_) => {}
        }

        Ok(())
    }

    async fn execute_trade(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        market_id: &str,
        side: &str,
        confidence: f64,
        bot_bet_size: f64,
        private_key: &str,
    ) -> Result<(), String> {
        let pm_client = PolymarketClient::new(private_key)
            .map_err(|e| format!("Failed to create client: {}", e))?;

        let token_id = market_id;
        
        // Use Kelly Criterion as max bound if confidence implies an edge
        let price_est = if side == "YES" { 0.5 } else { 0.5 }; // Simple default
        let edge = confidence - price_est;
        let kelly_pct = if edge > 0.0 && price_est > 0.0 {
            let odds = (1.0 - price_est) / price_est;
            (odds * confidence - (1.0 - confidence)) / odds
        } else {
            0.0
        };
        
        // Use bot's configured bet size as base, optionally scale by Kelly
        let size = if kelly_pct > 0.0 {
            bot_bet_size.max(1.0) // simplified to use bot config
        } else {
            bot_bet_size
        };

        let quote_side = if side == "YES" { "BUY" } else { "SELL" };
        let price = pm_client.get_quote(token_id, quote_side, size)
            .await
            .unwrap_or(0.5);

        let balance = pm_client.get_balance().await.unwrap_or(0.0);
        if balance < size * price {
            let msg = format!("Insufficient balance: {} < {}", balance, size * price);
            tracing::warn!("{}", msg);
            let _ = Self::log_activity(db, user_id, Some(bot_id), "WARNING", &msg).await;
            return Ok(());
        }

        let _order_request = polymarket::OrderRequest {
            token_id: token_id.to_string(),
            price,
            size,
            side: if side == "YES" { "BUY".to_string() } else { "SELL".to_string() },
        };

        let _order_id = format!("auto_{}", chrono::Utc::now().timestamp_millis());

        let msg = format!(
            "Would place order: {} {} @ {} (${})",
            side, size, price, size * price
        );

        tracing::info!("{}", msg);
        let _ = Self::log_activity(db, user_id, Some(bot_id), "INFO", &msg).await;

        let _ = queries::create_order(
            db,
            bot_id,
            user_id,
            market_id,
            side,
            price,
            size,
        ).await;

        Ok(())
    }

    async fn log_activity(
        db: &Db,
        user_id: i64,
        bot_id: Option<i64>,
        level: &str,
        message: &str,
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO activity_log (user_id, bot_id, level, message) VALUES (?, ?, ?, ?)"
        )
        .bind(user_id)
        .bind(bot_id)
        .bind(level)
        .bind(message)
        .execute(db.as_ref())
        .await
        .map_err(|e| format!("Failed to log activity: {}", e))?;

        Ok(())
    }
}

/// Start a bot by ID
/// MEGJEGYZÉS: Ez a függvény NEM indítja el a tényleges trading loop-ot.
/// A valódi indítást az api/bots.rs start_bot() végzi az orchestrator-on keresztül.
/// Ez a függvény megtartva a kompatibilitás miatt, de nem hívódik meg a normál flow-ban.
pub async fn start_bot(
    db: &Db,
    bot_id: i64,
    user_id: i64,
) -> Result<String, String> {
    let bot = queries::get_bot_by_id(db, bot_id, user_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Bot not found")?;

    // FIX: A credential-t a credential_cache-ből kell venni (CachedCredentials),
    // nem itt decryptálni hardkódolt jelszóval. Ez a függvény elavult.
    tracing::warn!(
        "executor::start_bot() called for bot {} - use orchestrator::start_bot() instead",
        bot_id
    );

    queries::update_bot_status(db, bot_id, user_id, "running")
        .await
        .map_err(|e| e.to_string())?;

    let msg = format!("Bot '{}' started with strategy '{}'", bot.name, bot.strategy_type);
    let _ = sqlx::query(
        "INSERT INTO activity_log (user_id, bot_id, level, message) VALUES (?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(bot_id)
    .bind("INFO")
    .bind(&msg)
    .execute(db.as_ref())
    .await;

    tracing::info!("Starting bot {} with strategy {}", bot.name, bot.strategy_type);

    Ok(format!("Bot '{}' started successfully", bot.name))
}

/// Stop a bot by ID
pub async fn stop_bot(
    db: &Db,
    bot_id: i64,
    user_id: i64,
) -> Result<String, String> {
    queries::update_bot_status(db, bot_id, user_id, "stopped")
        .await
        .map_err(|e| e.to_string())?;

    let bot = queries::get_bot_by_id(db, bot_id, user_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Bot not found")?;

    let msg = format!("Bot '{}' stopped", bot.name);
    let _ = sqlx::query(
        "INSERT INTO activity_log (user_id, bot_id, level, message) VALUES (?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(bot_id)
    .bind("INFO")
    .bind(&msg)
    .execute(db.as_ref())
    .await;

    tracing::info!("Stopped bot {}", bot_id);

    Ok(format!("Bot '{}' stopped successfully", bot.name))
}
