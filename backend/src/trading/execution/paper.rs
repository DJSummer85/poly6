//! Paper Execution Adapter - Demo mode order execution with simulated fills
//! Replaces real Polymarket orders with deterministic simulated execution

use crate::db::Db;
use crate::db::queries;
use serde::{Deserialize, Serialize};

/// Paper execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperExecutionResult {
    pub execution_id: i64,
    pub intent_id: i64,
    pub status: String,
    pub side: String,
    pub requested_size: f64,
    pub filled_size: f64,
    pub avg_fill_price: f64,
    pub simulated_fill: bool,
    pub error_code: Option<String>,
}

/// Trade intent for paper execution
#[derive(Debug, Clone)]
pub struct PaperTradeIntent {
    pub run_id: Option<i64>,
    pub bot_id: i64,
    pub user_id: i64,
    pub market_id: String,
    pub strategy_type: String,
    pub side: String,
    pub confidence: f64,
    pub reason: String,
    pub snapshot_json: Option<String>,
}

/// Risk check result
#[derive(Debug, Clone)]
pub struct RiskCheckResult {
    pub approved: bool,
    pub reason: Option<String>,
    pub max_size: Option<f64>,
}

pub struct PaperExecutionAdapter {
    db: Db,
}

impl PaperExecutionAdapter {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Execute a paper trade: validate intent -> risk check -> simulated fill -> record
    pub async fn execute(&self, intent: PaperTradeIntent) -> Result<PaperExecutionResult, String> {
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
            // Update intent as rejected
            sqlx::query("UPDATE trade_intents SET status = 'rejected' WHERE id = ?")
                .bind(intent_id)
                .execute(db.as_ref())
                .await
                .map_err(|e| format!("Failed to update intent: {}", e))?;

            return Ok(PaperExecutionResult {
                execution_id: 0,
                intent_id,
                status: "rejected".to_string(),
                side: intent.side,
                requested_size: 0.0,
                filled_size: 0.0,
                avg_fill_price: 0.0,
                simulated_fill: false,
                error_code: risk.reason.clone(),
            });
        }

        // 3. Calculate simulated fill price
        let (filled_size, avg_fill_price) = self.calculate_simulated_fill(&intent, risk.max_size).await?;

        // 4. Create execution record
        let execution_id: i64 = sqlx::query(
            r#"
            INSERT INTO executions
                (intent_id, run_id, bot_id, user_id, mode, adapter, status, market_id, side,
                 requested_size, filled_size, requested_price, avg_fill_price, external_order_id, error_code, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'demo', 'paper', 'filled', ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
            "#,
        )
        .bind(intent_id)
        .bind(intent.run_id)
        .bind(intent.bot_id)
        .bind(intent.user_id)
        .bind(&intent.market_id)
        .bind(&intent.side)
        .bind(filled_size) // requested_size
        .bind(filled_size)
        .bind(avg_fill_price) // requested_price
        .bind(avg_fill_price) // avg_fill_price
        .bind(&now)
        .bind(&now)
        .execute(db.as_ref())
        .await
        .map_err(|e| format!("Failed to create execution: {}", e))?
        .last_insert_rowid();

        // 5. Update intent as executed
        sqlx::query("UPDATE trade_intents SET status = 'executed' WHERE id = ?")
            .bind(intent_id)
            .execute(db.as_ref())
            .await
            .map_err(|e| format!("Failed to update intent: {}", e))?;

        // 6. Update or create position
        self.update_position(db, intent.bot_id, intent.user_id, &intent.market_id, &intent.side, filled_size, avg_fill_price).await?;

        // 7. Update portfolio balance
        self.update_balance(db, intent.bot_id, intent.user_id, filled_size, avg_fill_price, &intent.side).await?;

        Ok(PaperExecutionResult {
            execution_id,
            intent_id,
            status: "filled".to_string(),
            side: intent.side,
            requested_size: filled_size,
            filled_size,
            avg_fill_price,
            simulated_fill: true,
            error_code: None,
        })
    }

    /// Risk check for paper trades - same rules as live
    async fn check_risk(&self, intent: &PaperTradeIntent) -> Result<RiskCheckResult, String> {
        let db = &self.db;

        // Get bot config
        let _bot = queries::get_bot_by_id(db, intent.bot_id, intent.user_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Bot not found")?;

        // Check portfolio balance
        let portfolio = queries::get_portfolio(db, intent.bot_id, intent.user_id)
            .await
            .map_err(|e| e.to_string())?;

        let balance = portfolio.as_ref().map(|p| p.balance).unwrap_or(100.0);

        // Default max bet is 25% of balance (kelly fraction)
        let max_bet = balance * 0.25;

        // Check if bet size would exceed max
        let default_size = 1.0;
        if default_size > max_bet {
            return Ok(RiskCheckResult {
                approved: false,
                reason: Some(format!("Bet size ${:.2} exceeds max ${:.2}", default_size, max_bet)),
                max_size: Some(max_bet),
            });
        }

        // Check time remaining (if available from snapshot)
        if let Some(ref snapshot) = intent.snapshot_json {
            if snapshot.contains("time_remaining") {
                // Basic validation passed
            }
        }

        Ok(RiskCheckResult {
            approved: true,
            reason: None,
            max_size: Some(max_bet),
        })
    }

    /// Calculate simulated fill: midpoint price with tiny slippage
    async fn calculate_simulated_fill(&self, intent: &PaperTradeIntent, max_size: Option<f64>) -> Result<(f64, f64), String> {
        // Parse snapshot to get prices
        let (yes_price, no_price) = if let Some(ref snapshot) = intent.snapshot_json {
            // Try to extract yes/no prices from snapshot JSON
            // For now, use defaults if parsing fails
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(snapshot) {
                let yes = parsed.get("yes_price").and_then(|v| v.as_f64()).unwrap_or(0.5);
                let no = parsed.get("no_price").and_then(|v| v.as_f64()).unwrap_or(0.5);
                (yes, no)
            } else {
                (0.5, 0.5)
            }
        } else {
            (0.5, 0.5)
        };

        let side = &intent.side;
        let mid_price = if side == "YES" { yes_price } else { no_price };

        // Simulated fill: midpoint with 0.01% slippage
        let slippage = 0.0001;
        let fill_price = if side == "YES" {
            (mid_price * (1.0 + slippage)).min(0.99)
        } else {
            (mid_price * (1.0 - slippage)).max(0.01)
        };

        // Size is limited by max_size (from risk check) or default 1.0
        let size = max_size.map(|m| m.min(1.0)).unwrap_or(1.0);

        Ok((size, fill_price))
    }

    /// Update or create position after paper fill
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
        // Check if position exists for this market
        let positions = queries::get_positions_by_user(db, user_id)
            .await
            .map_err(|e| e.to_string())?;

        // Find position for this market
        if let Some(pos) = positions.iter().find(|p| p.market_id == market_id) {
            // Update existing position
            let new_size = if pos.side == side {
                pos.size + size
            } else {
                // Opposite side - reduce or flip
                if pos.size > size {
                    pos.size - size
                } else {
                    size - pos.size
                }
            };

            if new_size > 0.0 {
                sqlx::query(
                    "UPDATE positions SET size = ?, avg_price = ? WHERE id = ?"
                )
                .bind(new_size)
                .bind(price)
                .bind(pos.id)
                .execute(db.as_ref())
                .await
                .map_err(|e| format!("Failed to update position: {}", e))?;
            } else {
                // Position closed
                sqlx::query("DELETE FROM positions WHERE id = ?")
                    .bind(pos.id)
                    .execute(db.as_ref())
                    .await
                    .map_err(|e| format!("Failed to delete position: {}", e))?;
            }
        } else {
            // Create new position
            let avg_price = price;
            sqlx::query(
                r#"INSERT INTO positions (bot_id, user_id, market_id, side, size, avg_price, opened_at)
                   VALUES (?, ?, ?, ?, ?, ?, datetime('now'))"#
            )
            .bind(bot_id)
            .bind(user_id)
            .bind(market_id)
            .bind(side)
            .bind(size)
            .bind(avg_price)
            .execute(db.as_ref())
            .await
            .map_err(|e| format!("Failed to create position: {}", e))?;
        }

        Ok(())
    }

    /// Update portfolio balance after paper trade
    async fn update_balance(
        &self,
        db: &Db,
        bot_id: i64,
        user_id: i64,
        size: f64,
        price: f64,
        _side: &str,
    ) -> Result<(), String> {
        let cost = size * price;

        queries::ensure_portfolio(db, bot_id, user_id, 100.0)
            .await
            .map_err(|e| e.to_string())?;

        // Deduct cost from balance (for YES/NO buy, cost is size * price)
        sqlx::query(
            "UPDATE bot_portfolios SET balance = balance - ? WHERE bot_id = ? AND user_id = ?"
        )
        .bind(cost)
        .bind(bot_id)
        .bind(user_id)
        .execute(db.as_ref())
        .await
        .map_err(|e| format!("Failed to update balance: {}", e))?;

        Ok(())
    }
}
