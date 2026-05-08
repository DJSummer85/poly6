//! Strategy Lab API - Backend for strategy testing
//! POST/GET strategy-tests with full event timeline and results

use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use axum::http::StatusCode;

use crate::api::AppState;
use crate::trading::execution::paper::{PaperExecutionAdapter, PaperTradeIntent};

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/strategies", get(list_strategies))
        .route("/strategy-tests", post(create_strategy_test))
        .route("/strategy-tests/:id", get(get_strategy_test))
        .route("/strategy-tests/:id/events", get(get_strategy_test_events))
        .route("/strategy-tests/:id/performance", get(get_strategy_test_performance))
}

#[derive(Serialize)]
pub struct StrategiesResponse {
    pub strategies: Vec<StrategyInfo>,
}

#[derive(Serialize)]
pub struct StrategyInfo {
    id: String,
    name: String,
    description: String,
    params: Vec<String>,
}

/// List all available strategies
pub async fn list_strategies() -> Json<StrategiesResponse> {
    Json(StrategiesResponse {
        strategies: vec![
            StrategyInfo { id: "window_delta".into(), name: "Window Delta".into(), description: "BTC price vs window opening".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "momentum".into(), name: "Momentum".into(), description: "BTC 24h change momentum".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "trend".into(), name: "Trend".into(), description: "BTC price trend following".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "volatility".into(), name: "Volatility".into(), description: "Volatility breakout".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "sniper".into(), name: "Sniper".into(), description: "Ultra-low entry sniper".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "contrarian".into(), name: "Contrarian".into(), description: "Counter-trend reversal".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "mean_reversion".into(), name: "Mean Reversion".into(), description: "Price deviation from mean".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "binance_velocity".into(), name: "Binance Velocity".into(), description: "BTC velocity + acceleration".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "fair_value".into(), name: "Fair Value".into(), description: "Delta-based fair probability".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
            StrategyInfo { id: "oracle_lag".into(), name: "Oracle Lag".into(), description: "CLOB vs spot delta".into(), params: vec!["min_delta".into(), "min_price".into(), "max_price".into()] },
        ],
    })
}

#[derive(Deserialize)]
pub struct CreateStrategyTestRequest {
    pub strategy_type: String,
    pub params: Option<String>,
    pub market_id: String,
    pub initial_balance: Option<f64>,
    pub mode: Option<String>,
}

type TestResult = (StatusCode, Json<serde_json::Value>);

/// Create a new strategy test (runs immediately with current market data)
pub async fn create_strategy_test(
    State(state): State<AppState>,
    Json(req): Json<CreateStrategyTestRequest>,
) -> TestResult {
    let db = &state.db;
    let market_service = &state.market_service;
    let now = chrono::Utc::now().to_rfc3339();

    let initial_balance = req.initial_balance.unwrap_or(100.0);
    let mode = req.mode.unwrap_or_else(|| "demo".to_string());

    // Create strategy test record in bot_runs
    let test_id: i64 = match sqlx::query(
        r#"
        INSERT INTO bot_runs (bot_id, user_id, mode, status, initial_balance, created_at)
        VALUES (0, 0, ?, 'running', ?, ?)
        "#,
    )
    .bind(&mode)
    .bind(initial_balance)
    .bind(&now)
    .execute(db.as_ref())
    .await
    {
        Ok(r) => r.last_insert_rowid(),
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to create test: {}", e)
            })));
        }
    };

    // Fetch market snapshot
    let snapshot = match market_service.get_snapshot(&req.market_id).await {
        Ok(s) => s,
        Err(e) => {
            sqlx::query("UPDATE bot_runs SET status = 'failed' WHERE id = ?")
                .bind(test_id)
                .execute(db.as_ref())
                .await
                .ok();
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Failed to fetch market: {}", e)
            })));
        }
    };

    let snapshot_json = serde_json::to_string(&snapshot).unwrap_or_default();

    // Evaluate strategy
    let strategy_executor = crate::trading::bot_executor::strategies::StrategyExecutor::new(
        &req.strategy_type,
        req.params.as_deref().unwrap_or("{}"),
    );

    let context = snapshot.to_strategy_context();
    let signal = strategy_executor.evaluate_with_context(context.clone());

    let side = match &signal {
        crate::trading::bot_executor::strategies::Signal::Yes(_) => "YES",
        crate::trading::bot_executor::strategies::Signal::No(_) => "NO",
        crate::trading::bot_executor::strategies::Signal::Hold(_) => "HOLD",
    };

    let confidence = match &signal {
        crate::trading::bot_executor::strategies::Signal::Yes(c) => *c,
        crate::trading::bot_executor::strategies::Signal::No(c) => *c,
        crate::trading::bot_executor::strategies::Signal::Hold(_) => 0.0,
    };

    let reason = match &signal {
        crate::trading::bot_executor::strategies::Signal::Hold(r) => r.clone(),
        _ => "strategy signal".to_string(),
    };

    // Record trade intent
    let intent_id: i64 = match sqlx::query(
        r#"
        INSERT INTO trade_intents
            (run_id, bot_id, user_id, market_id, strategy_type, side, confidence, reason, snapshot_json, status, created_at)
        VALUES (?, 0, 0, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(test_id)
    .bind(&req.market_id)
    .bind(&req.strategy_type)
    .bind(side)
    .bind(confidence)
    .bind(&reason)
    .bind(&snapshot_json)
    .bind("pending")
    .bind(&now)
    .execute(db.as_ref())
    .await
    {
        Ok(r) => r.last_insert_rowid(),
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to create intent: {}", e)
            })));
        }
    };

    // Execute paper trade if not hold
    let mut final_balance = initial_balance;
    let mut status = "completed".to_string();
    let mut total_trades = 0;
    let mut winning_trades = 0;
    let mut losing_trades = 0;
    let mut total_pnl = 0.0;

    if !matches!(signal, crate::trading::bot_executor::strategies::Signal::Hold(_)) {
        let intent = PaperTradeIntent {
            run_id: Some(test_id),
            bot_id: 0,
            user_id: 0,
            market_id: req.market_id.clone(),
            strategy_type: req.strategy_type.clone(),
            side: side.to_string(),
            confidence,
            reason: reason.clone(),
            snapshot_json: Some(snapshot_json.clone()),
        };

        let adapter = PaperExecutionAdapter::new(db.clone());
        match adapter.execute(intent).await {
            Ok(result) => {
                total_trades = 1;
                if result.filled_size > 0.0 {
                    final_balance = initial_balance - (result.filled_size * result.avg_fill_price);
                    if final_balance > initial_balance {
                        winning_trades = 1;
                        total_pnl = final_balance - initial_balance;
                    } else {
                        losing_trades = 1;
                        total_pnl = final_balance - initial_balance;
                    }
                }
            }
            Err(_) => {
                sqlx::query("UPDATE trade_intents SET status = 'rejected' WHERE id = ?")
                    .bind(intent_id)
                    .execute(db.as_ref())
                    .await
                    .ok();
                status = "rejected".to_string();
            }
        }
    }

    sqlx::query(
        r#"UPDATE bot_runs SET status = ?, final_balance = ?, total_trades = ?,
           winning_trades = ?, losing_trades = ?, total_pnl = ? WHERE id = ?"#
    )
    .bind(&status)
    .bind(final_balance)
    .bind(total_trades)
    .bind(winning_trades)
    .bind(losing_trades)
    .bind(total_pnl)
    .bind(test_id)
    .execute(db.as_ref())
    .await
    .ok();

    (StatusCode::CREATED, Json(serde_json::json!({
        "id": test_id,
        "strategy_type": req.strategy_type,
        "market_id": req.market_id,
        "status": status,
        "initial_balance": initial_balance,
        "final_balance": final_balance,
        "signal": {
            "side": side,
            "confidence": confidence,
            "reason": reason
        },
        "total_trades": total_trades,
        "total_pnl": total_pnl
    })))
}

/// Get strategy test by ID
pub async fn get_strategy_test(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> TestResult {
    let row: Option<(String, String, String, f64, Option<f64>, i32, i32, i32, f64)> = sqlx::query_as(
        "SELECT strategy_type, market_id, status, initial_balance, final_balance, total_trades, winning_trades, losing_trades, total_pnl FROM bot_runs WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(state.db.as_ref())
    .await
    .ok()
    .flatten();

    match row {
        Some((strategy_type, market_id, status, initial_balance, final_balance, total_trades, winning_trades, losing_trades, total_pnl)) => {
            (StatusCode::OK, Json(serde_json::json!({
                "id": id,
                "strategy_type": strategy_type,
                "market_id": market_id,
                "status": status,
                "initial_balance": initial_balance,
                "final_balance": final_balance,
                "total_trades": total_trades,
                "winning_trades": winning_trades,
                "losing_trades": losing_trades,
                "total_pnl": total_pnl
            })))
        }
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Test not found"}))),
    }
}

/// Get events for a strategy test
pub async fn get_strategy_test_events(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let intents: Vec<(i64, String, String, f64, String, String, String, String)> = sqlx::query_as(
        "SELECT id, side, strategy_type, confidence, reason, status, snapshot_json, created_at FROM trade_intents WHERE run_id = ? ORDER BY created_at"
    )
    .bind(id)
    .fetch_all(state.db.as_ref())
    .await
    .ok()
    .unwrap_or_default();

    let executions: Vec<(i64, String, String, f64, f64, String, String)> = sqlx::query_as(
        "SELECT id, side, status, filled_size, avg_fill_price, error_code, created_at FROM executions WHERE run_id = ? ORDER BY created_at"
    )
    .bind(id)
    .fetch_all(state.db.as_ref())
    .await
    .ok()
    .unwrap_or_default();

    Json(serde_json::json!({
        "intents": intents,
        "executions": executions
    }))
}

/// Get performance metrics for a strategy test
pub async fn get_strategy_test_performance(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> TestResult {
    let row: Option<(f64, Option<f64>, i32, i32, i32, f64)> = sqlx::query_as(
        "SELECT initial_balance, final_balance, total_trades, winning_trades, losing_trades, total_pnl FROM bot_runs WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(state.db.as_ref())
    .await
    .ok()
    .flatten();

    match row {
        Some((initial_balance, final_balance, total_trades, winning_trades, losing_trades, total_pnl)) => {
            let win_rate = if total_trades > 0 {
                winning_trades as f64 / total_trades as f64
            } else {
                0.0
            };

            let roi = if initial_balance > 0.0 {
                total_pnl / initial_balance
            } else {
                0.0
            };

            (StatusCode::OK, Json(serde_json::json!({
                "initial_balance": initial_balance,
                "final_balance": final_balance.unwrap_or(initial_balance),
                "total_trades": total_trades,
                "winning_trades": winning_trades,
                "losing_trades": losing_trades,
                "win_rate": win_rate,
                "total_pnl": total_pnl,
                "roi": roi
            })))
        }
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Test not found"}))),
    }
}
