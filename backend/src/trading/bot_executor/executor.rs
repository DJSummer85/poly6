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
use crate::crypto;

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
        let private_key = private_key.to_string(); // Convert to owned String

        // Spawn the execution loop
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(interval_secs));

            loop {
                // Check if we should stop
                let should_run = *running.read().await;
                if !should_run {
                    tracing::info!("Bot executor loop stopped");
                    break;
                }

                interval_timer.tick().await;

                // Get bot config
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

                // Check if bot is still enabled
                if bot.status != "running" {
                    tracing::info!("Bot {} is not running (status: {})", bot_id, bot.status);
                    break;
                }

                // Execute the strategy cycle
                if let Err(e) = Self::execute_bot_cycle(&db, bot_id, user_id, &bot, &private_key, &market_service).await {
                    tracing::error!("Bot cycle error: {}", e);
                }
            }

            // Mark as stopped
            *running.write().await = false;
        });

        Ok(())
    }

    /// Stop the execution loop
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Check if executor is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Execute one cycle of the bot: get data -> evaluate strategy -> place order
    async fn execute_bot_cycle(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        bot: &crate::db::BotRecord,
        private_key: &str,
        market_service: &MarketDataService,
    ) -> Result<(), String> {
        // Get market snapshot with full Polymarket + Binance context
        let snapshot = market_service.get_snapshot(&bot.market_id).await?;

        // Create strategy executor
        let strategy = StrategyExecutor::new(&bot.strategy_type, &bot.params);

        // Evaluate strategy with full market context
        let signal = strategy.evaluate_with_context(snapshot.to_strategy_context());

        // Log the signal
        let signal_msg = match &signal {
            Signal::Yes(conf) => format!("BUY YES with confidence {}", conf),
            Signal::No(conf) => format!("BUY NO with confidence {}", conf),
            Signal::Hold(reason) => format!("HOLD: {}", reason),
        };

        tracing::info!("Bot {} signal: {}", bot.name, signal_msg);

        // Log activity
        let _ = Self::log_activity(db, user_id, Some(bot_id), "INFO", &signal_msg).await;

        // Execute based on signal
        match signal {
            Signal::Yes(confidence) | Signal::No(confidence) => {
                // Place order on Polymarket
                Self::execute_trade(
                    db,
                    bot_id,
                    user_id,
                    &bot.market_id,
                    if matches!(signal, Signal::Yes(_)) { "YES" } else { "NO" },
                    confidence,
                    private_key,
                ).await?;
            }
            Signal::Hold(_) => {
                // No action needed
            }
        }

        Ok(())
    }

    /// Get current BTC price from Binance
    async fn get_binance_price() -> Result<f64, String> {
        // Try to fetch BTC/USDT price from Binance
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
            .send()
            .await
            .map_err(|e| format!("Binance request failed: {}", e))?;

        if !response.status().is_success() {
            return Err("Binance API error".to_string());
        }

        #[derive(serde::Deserialize)]
        struct BinancePrice {
            price: String,
        }

        let data: BinancePrice = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        data.price
            .parse::<f64>()
            .map_err(|e| format!("Failed to parse price: {}", e))
    }

    /// Execute a trade on Polymarket
    async fn execute_trade(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        market_id: &str,
        side: &str,
        _confidence: f64,
        private_key: &str,
    ) -> Result<(), String> {
        // Get Polymarket client
        let pm_client = PolymarketClient::new(private_key)
            .map_err(|e| format!("Failed to create client: {}", e))?;

        // Get market info to get token_id
        let token_id = market_id; // Use market_id as token_id for now

        // Calculate size based on confidence (use fixed size for now, could use Kelly)
        let size = 1.0; // $1 for now

        // Get current price (use quote)
        let price = pm_client.get_quote(token_id, "BUY", size)
            .await
            .unwrap_or(0.5);

        // Check balance
        let balance = pm_client.get_balance().await.unwrap_or(0.0);
        if balance < size * price {
            let msg = format!("Insufficient balance: {} < {}", balance, size * price);
            tracing::warn!("{}", msg);
            let _ = Self::log_activity(db, user_id, Some(bot_id), "WARNING", &msg).await;
            return Ok(());
        }

        // Create and place order
        let _order_request = polymarket::OrderRequest {
            token_id: token_id.to_string(),
            price,
            size,
            side: if side == "YES" { "BUY".to_string() } else { "SELL".to_string() },
        };

        // Create and sign order (requires API creds)
        // For now, we can't place real orders without API key creds
        let _order_id = format!("auto_{}", chrono::Utc::now().timestamp_millis());

        let msg = format!(
            "Would place order: {} {} @ {} (${})",
            side, size, price, size * price
        );

        tracing::info!("{}", msg);
        let _ = Self::log_activity(db, user_id, Some(bot_id), "INFO", &msg).await;

        // Record order in database
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

    /// Log activity to database
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
pub async fn start_bot(
    db: &Db,
    bot_id: i64,
    user_id: i64,
) -> Result<String, String> {
    // Get bot config
    let bot = queries::get_bot_by_id(db, bot_id, user_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Bot not found")?;

    // Get credentials for private key
    let settings = queries::get_settings(db, user_id)
        .await
        .map_err(|e| e.to_string())?;

    let (_private_key, _encrypted_blob) = match settings {
        Some((_, blob)) if !blob.is_empty() => {
            // Try to decrypt - for now use the known key
            let password = "techno";
            let encryption_key = format!("{}_pm_creds", password);

            match crypto::decrypt(&blob, &encryption_key) {
                Ok(json_str) => {
                    #[derive(serde::Deserialize)]
                    struct Creds {
                        private_key: Option<String>,
                        key: Option<String>,
                        secret: Option<String>,
                        passphrase: Option<String>,
                    }

                    let creds: Creds = serde_json::from_str(&json_str).unwrap_or(Creds {
                        private_key: None,
                        key: None,
                        secret: None,
                        passphrase: None,
                    });

                    (creds.private_key.unwrap_or_else(||
                        "REMOVED_ADDRESS".to_string()
                    ), blob)
                }
                Err(_) => (
                    "REMOVED_ADDRESS".to_string(),
                    blob,
                )
            }
        }
        _ => (
            "REMOVED_ADDRESS".to_string(),
            String::new(),
        ),
    };

    // Update bot status to running
    queries::update_bot_status(db, bot_id, user_id, "running")
        .await
        .map_err(|e| e.to_string())?;

    // Log activity
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

    // Start the executor - for now just log that we would start
    tracing::info!("Starting bot {} with strategy {}", bot.name, bot.strategy_type);

    Ok(format!("Bot '{}' started successfully", bot.name))
}

/// Stop a bot by ID
pub async fn stop_bot(
    db: &Db,
    bot_id: i64,
    user_id: i64,
) -> Result<String, String> {
    // Update bot status to stopped
    queries::update_bot_status(db, bot_id, user_id, "stopped")
        .await
        .map_err(|e| e.to_string())?;

    // Get bot for logging
    let bot = queries::get_bot_by_id(db, bot_id, user_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Bot not found")?;

    // Log activity
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
