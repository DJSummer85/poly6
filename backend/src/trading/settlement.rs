//! Settlement Service - Handles market settlement for demo mode
//! When a market closes, resolves positions based on BTC price vs price_to_beat

use crate::db::Db;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const GAMMA_API: &str = "https://gamma-api.polymarket.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementResult {
    pub market_id: String,
    pub outcome: String,
    pub price_to_beat: Option<f64>,
    pub final_btc_price: Option<f64>,
    pub settled: bool,
    pub total_pnl: f64,
    pub winning_positions: i32,
    pub losing_positions: i32,
}

pub struct SettlementService {
    http_client: reqwest::Client,
}

impl SettlementService {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    /// Attempt to settle a market: check if expired, resolve outcome, update positions
    pub async fn try_settle(&self, db: &Db, market_id: &str, btc_price: f64) -> Result<SettlementResult, String> {
        let mut result = SettlementResult {
            market_id: market_id.to_string(),
            outcome: String::new(),
            price_to_beat: None,
            final_btc_price: Some(btc_price),
            settled: false,
            total_pnl: 0.0,
            winning_positions: 0,
            losing_positions: 0,
        };

        // Get market info from gamma API
        let market_data = self.fetch_market_data(market_id).await?;

        // Check if market is closed/settled
        let end_date = market_data.get("endDate").and_then(|v| v.as_str());
        if let Some(date_str) = end_date {
            if let Ok(expire_dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                if expire_dt.timestamp() > now {
                    // Market not yet expired
                    return Err("Market not yet expired".to_string());
                }
            }
        }

        // Get price_to_beat
        let price_to_beat = market_data.get("priceToBeat")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                // Try to get from trading price
                market_data.get("outcomePrices")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
            });

        result.price_to_beat = price_to_beat;

        // Determine outcome: YES wins if btc_price >= price_to_beat
        let outcome = if let Some(ptb) = price_to_beat {
            if btc_price >= ptb {
                "YES".to_string()
            } else {
                "NO".to_string()
            }
        } else {
            // No price_to_beat, can't determine
            return Err("No price_to_beat found".to_string());
        };

        result.outcome = outcome.clone();

        // Find all positions for this market and settle them
        let positions = sqlx::query_as::<_, (i64, i64, String, String, f64, f64)>(
            "SELECT id, bot_id, side, market_id, size, avg_price FROM positions WHERE market_id = ?"
        )
        .bind(market_id)
        .fetch_all(db.as_ref())
        .await
        .map_err(|e| format!("Failed to fetch positions: {}", e))?;

        let mut total_pnl = 0.0;
        let mut winning = 0;
        let mut losing = 0;

        for (pos_id, bot_id, side, _market_id, size, avg_price) in positions {
            let pnl = if side == outcome {
                // Winning position: profit = size * (1 - avg_price) for YES, size * avg_price for NO
                if side == "YES" {
                    size * (1.0 - avg_price)
                } else {
                    size * avg_price
                }
            } else {
                // Losing position: lose the cost
                -(size * avg_price)
            };

            // Update balance for this bot
            sqlx::query(
                "UPDATE bot_portfolios SET balance = balance + ? WHERE bot_id = ?"
            )
            .bind(pnl)
            .bind(bot_id)
            .execute(db.as_ref())
            .await
            .map_err(|e| format!("Failed to update balance: {}", e))?;

            // Update bot stats
            if pnl > 0.0 {
                sqlx::query(
                    "UPDATE bot_portfolios SET winning_trades = winning_trades + 1, total_pnl = total_pnl + ? WHERE bot_id = ?"
                )
                .bind(pnl)
                .bind(bot_id)
                .execute(db.as_ref())
                .await
                .map_err(|e| format!("Failed to update stats: {}", e))?;
                winning += 1;
            } else {
                sqlx::query(
                    "UPDATE bot_portfolios SET losing_trades = losing_trades + 1, total_pnl = total_pnl + ? WHERE bot_id = ?"
                )
                .bind(pnl)
                .bind(bot_id)
                .execute(db.as_ref())
                .await
                .map_err(|e| format!("Failed to update stats: {}", e))?;
                losing += 1;
            }

            // Delete the position
            sqlx::query("DELETE FROM positions WHERE id = ?")
                .bind(pos_id)
                .execute(db.as_ref())
                .await
                .map_err(|e| format!("Failed to delete position: {}", e))?;

            // Record settlement
            sqlx::query(
                r#"INSERT INTO activity_log (user_id, bot_id, level, message) VALUES (0, ?, 'INFO', ?)"#
            )
            .bind(bot_id)
            .bind(&format!("Settled market {}: {} won ${:.2}", market_id, side, pnl))
            .execute(db.as_ref())
            .await
            .ok();

            total_pnl += pnl;
        }

        result.settled = true;
        result.total_pnl = total_pnl;
        result.winning_positions = winning;
        result.losing_positions = losing;

        Ok(result)
    }

    async fn fetch_market_data(&self, market_id: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        let response = self
            .http_client
            .get(format!("{}/markets/{}", GAMMA_API, market_id))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Gamma API request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Gamma API error: {}", response.status()));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(obj) = data.as_object() {
            Ok(obj.clone())
        } else {
            Err("Invalid market data format".to_string())
        }
    }
}

impl Default for SettlementService {
    fn default() -> Self {
        Self::new()
    }
}
