//! Live Execution Adapter - Real Polymarket CLOB order execution
//! Uses PolymarketClient for live trading with actual orders

use crate::db::Db;
use crate::trading::polymarket::{OrderRequest, PolymarketClient};
use serde::{Deserialize, Serialize};

const CLOB_API: &str = "https://clob.polymarket.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveExecutionResult {
    pub execution_id: i64,
    pub intent_id: i64,
    pub status: String,
    pub side: String,
    pub requested_size: f64,
    pub filled_size: f64,
    pub avg_fill_price: f64,
    pub order_id: Option<String>,
    pub error_code: Option<String>,
}

pub struct LiveExecutionAdapter {
    db: Db,
    pub client: PolymarketClient,
}

impl LiveExecutionAdapter {
    pub fn new(db: Db, client: PolymarketClient) -> Self {
        Self { db, client }
    }

    /// Execute a live trade: validate intent -> risk check -> CLOB order -> record
    pub async fn execute(&self, intent: crate::trading::execution::paper::PaperTradeIntent) -> Result<LiveExecutionResult, String> {
        let db = &self.db;
        let now = chrono::Utc::now().to_rfc3339();

        // 1. Create trade_intent record
        let intent_id: i64 = sqlx::query(
            r#"
            INSERT INTO trade_intents
                (run_id, bot_id, user_id, market_id, strategy_type, side, confidence, reason, snapshot_json, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
            "#,
        )
        .bind(intent.run_id)
        .bind(intent.bot_id)
        .bind(intent.user_id)
        .bind(&intent.market_id)
        .bind(&intent.strategy_type)
        .bind(&intent.side)
        .bind(intent.confidence)
        .bind(&intent.reason)
        .bind(&intent.snapshot_json)
        .bind(&now)
        .execute(db.as_ref())
        .await
        .map_err(|e| format!("Failed to create trade_intent: {}", e))?
        .last_insert_rowid();

        // 2. Risk check
        let risk = self.check_risk(&intent).await?;
        if !risk.approved {
            sqlx::query("UPDATE trade_intents SET status = 'rejected' WHERE id = ?")
                .bind(intent_id)
                .execute(db.as_ref())
                .await
                .map_err(|e| format!("Failed to update intent: {}", e))?;

            return Ok(LiveExecutionResult {
                execution_id: 0,
                intent_id,
                status: "rejected".to_string(),
                side: intent.side,
                requested_size: 0.0,
                filled_size: 0.0,
                avg_fill_price: 0.0,
                order_id: None,
                error_code: risk.reason.clone(),
            });
        }

        // 3. Get token_id for the market
        let token_id = self.get_token_id(&intent.market_id).await?;

        // 4. Submit CLOB order
        let order_request = OrderRequest {
            token_id: token_id.clone(),
            side: if intent.side == "YES" { "BUY".to_string() } else { "SELL".to_string() },
            size: risk.max_size.unwrap_or(1.0),
            price: self.estimate_price(&intent).await?,
        };

        let order_response = self.client.create_order_v2(&order_request, false).await;

        let (status, filled_size, avg_fill_price, order_id, error_code) = match order_response {
            Ok(resp) => {
                let filled = resp.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let price = resp.get("price").and_then(|v| v.as_f64()).unwrap_or(order_request.price);
                let oid = resp.get("orderId").or_else(|| resp.get("order_id")).and_then(|v| v.as_str()).map(String::from);
                (
                    "filled".to_string(),
                    filled,
                    price,
                    oid,
                    None,
                )
            }
            Err(e) => {
                let err_msg = e.to_string();
                ("failed".to_string(), 0.0, 0.0, None, Some(err_msg))
            }
        };

        // 5. Create execution record
        let execution_id: i64 = sqlx::query(
            r#"
            INSERT INTO executions
                (intent_id, run_id, bot_id, user_id, mode, adapter, status, market_id, token_id, side,
                 requested_size, filled_size, requested_price, avg_fill_price, external_order_id, error_code, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'live', 'clob', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(intent_id)
        .bind(intent.run_id)
        .bind(intent.bot_id)
        .bind(intent.user_id)
        .bind(&status)
        .bind(&intent.market_id)
        .bind(&token_id)
        .bind(&intent.side)
        .bind(risk.max_size.unwrap_or(1.0))
        .bind(filled_size)
        .bind(order_request.price)
        .bind(avg_fill_price)
        .bind(&order_id)
        .bind(&error_code)
        .bind(&now)
        .bind(&now)
        .execute(db.as_ref())
        .await
        .map_err(|e| format!("Failed to create execution: {}", e))?
        .last_insert_rowid();

        // 6. Update intent
        sqlx::query("UPDATE trade_intents SET status = ? WHERE id = ?")
            .bind(if status == "filled" { "executed" } else { "failed" })
            .bind(intent_id)
            .execute(db.as_ref())
            .await
            .map_err(|e| format!("Failed to update intent: {}", e))?;

        // 7. Update position if filled
        if filled_size > 0.0 {
            self.update_position(db, intent.bot_id, intent.user_id, &intent.market_id, &intent.side, filled_size, avg_fill_price).await?;
        }

        Ok(LiveExecutionResult {
            execution_id,
            intent_id,
            status,
            side: intent.side,
            requested_size: risk.max_size.unwrap_or(1.0),
            filled_size,
            avg_fill_price,
            order_id,
            error_code,
        })
    }

    /// Risk check for live trades - same rules as paper but with real balance
    async fn check_risk(&self, intent: &crate::trading::execution::paper::PaperTradeIntent) -> Result<crate::trading::execution::paper::RiskCheckResult, String> {
        let db = &self.db;

        // Get bot config
        let bot = crate::db::queries::get_bot_by_id(db, intent.bot_id, intent.user_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Bot not found")?;

        // Check live balance via PolymarketClient
        let balance = self.client.get_balance().await.unwrap_or(0.0);

        // Max bet is 25% of balance (kelly fraction)
        let max_bet = balance * 0.25;

        if max_bet < 0.01 {
            return Ok(crate::trading::execution::paper::RiskCheckResult {
                approved: false,
                reason: Some(format!("Insufficient balance: ${:.2}", balance)),
                max_size: None,
            });
        }

        let default_size = 1.0;
        if default_size > max_bet {
            return Ok(crate::trading::execution::paper::RiskCheckResult {
                approved: false,
                reason: Some(format!("Bet size ${:.2} exceeds max ${:.2}", default_size, max_bet)),
                max_size: Some(max_bet),
            });
        }

        Ok(crate::trading::execution::paper::RiskCheckResult {
            approved: true,
            reason: None,
            max_size: Some(max_bet.min(default_size)),
        })
    }

    /// Get token_id for a market from Gamma API
    async fn get_token_id(&self, market_id: &str) -> Result<String, String> {
        let gamma_url = format!("https://gamma-api.polymarket.com/markets/{}", market_id);
        let response = self.client.http_client()
            .get(&gamma_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Gamma API request failed: {}", e))?;

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        // Try to get clobTokenIds
        if let Some(token_ids) = data.get("clobTokenIds").and_then(|v| v.as_str()) {
            // Token IDs are comma-separated: "token1,token2"
            if let Some(first) = token_ids.split(',').next() {
                return Ok(first.to_string());
            }
        }

        Err("No clobTokenIds found for market".to_string())
    }

    /// Estimate price from snapshot or use midpoint from CLOB
    async fn estimate_price(&self, intent: &crate::trading::execution::paper::PaperTradeIntent) -> Result<f64, String> {
        if let Some(ref snapshot) = intent.snapshot_json {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(snapshot) {
                if intent.side == "YES" {
                    return Ok(parsed.get("yes_price").and_then(|v| v.as_f64()).unwrap_or(0.5));
                } else {
                    return Ok(parsed.get("no_price").and_then(|v| v.as_f64()).unwrap_or(0.5));
                }
            }
        }
        Ok(0.5) // Default midpoint
    }

    /// Update or create position after live fill
    async fn update_position(
        &self,
        db: &Db,
        bot_id: i64,
        user_id: i64,
        market_id: &str,
        side: &str,
        size: f64,
        price: f64,
    ) -> Result<(), String> {
        let positions = crate::db::queries::get_positions_by_user(db, user_id)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(pos) = positions.iter().find(|p| p.market_id == market_id) {
            let new_size = if pos.side == side {
                pos.size + size
            } else {
                if pos.size > size { pos.size - size } else { size - pos.size }
            };

            if new_size > 0.0 {
                sqlx::query("UPDATE positions SET size = ?, avg_price = ? WHERE id = ?")
                    .bind(new_size)
                    .bind(price)
                    .bind(pos.id)
                    .execute(db.as_ref())
                    .await
                    .map_err(|e| format!("Failed to update position: {}", e))?;
            } else {
                sqlx::query("DELETE FROM positions WHERE id = ?")
                    .bind(pos.id)
                    .execute(db.as_ref())
                    .await
                    .map_err(|e| format!("Failed to delete position: {}", e))?;
            }
        } else {
            sqlx::query(
                r#"INSERT INTO positions (bot_id, user_id, market_id, side, size, avg_price, opened_at)
                   VALUES (?, ?, ?, ?, ?, ?, datetime('now'))"#
            )
            .bind(bot_id)
            .bind(user_id)
            .bind(market_id)
            .bind(side)
            .bind(size)
            .bind(price)
            .execute(db.as_ref())
            .await
            .map_err(|e| format!("Failed to create position: {}", e))?;
        }

        Ok(())
    }
}
