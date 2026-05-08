//! System monitoring endpoints
//!
//! Provides system status, bot health, and activity logs

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::queries;
use crate::middleware::auth::Claims;
use super::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Get system status (binance, balance, etc.)
pub async fn get_system_status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();

    let user_id = claims.user_id;

    // Check Binance connection
    let binance_client = state.binance_client.read().await;
    let binance_connected = binance_client.is_some();
    let btc_price = match binance_client.as_ref() {
        Some(client) => client.get_current_price().await,
        None => None,
    };

    // Check credentials: settings.encrypted_blob OR api_keys table
    let settings = queries::get_settings(&db, user_id).await.ok().flatten();
    let has_blob_creds = match &settings {
        Some((_key, blob)) => !blob.is_empty(),
        None => false,
    };

    // Also check api_keys table for individual key entries
    let has_api_keys = match queries::get_api_keys(&db, user_id).await {
        Ok(keys) => keys.iter().any(|k| k.is_valid),
        Err(_) => false,
    };

    let has_creds = has_blob_creds || has_api_keys;

    // Get bot count
    let bots = queries::get_bots_by_user(&db, user_id).await.unwrap_or_default();
    let running_bots = bots.iter().filter(|b| b.status == "running").count();
    let total_bots = bots.len();

    #[derive(Serialize)]
    struct SystemStatus {
        binance_connected: bool,
        btc_price: Option<f64>,
        has_polymarket_credentials: bool,
        total_bots: usize,
        running_bots: usize,
    }

    Json(SystemStatus {
        binance_connected,
        btc_price,
        has_polymarket_credentials: has_creds,
        total_bots,
        running_bots,
    }).into_response()
}

/// Get activity logs for a user or bot
#[derive(Debug, Serialize, Deserialize)]
pub struct GetLogsRequest {
    pub bot_id: Option<i64>,
    pub level: Option<String>,
    pub limit: Option<i64>,
}

pub async fn get_logs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<GetLogsRequest>,
) -> Response {
    let db = state.db();

    let user_id = claims.user_id;
    let limit = params.limit.unwrap_or(50);

    // Build query based on filters
    let mut query = String::from(
        "SELECT id, user_id, bot_id, level, message, metadata, created_at FROM activity_log WHERE user_id = ?"
    );

    if params.bot_id.is_some() {
        query.push_str(" AND bot_id = ?");
    }

    if params.level.is_some() {
        query.push_str(" AND level = ?");
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ?");

    // Execute query
    let mut q = sqlx::query(&query).bind(user_id);

    if let Some(bot_id) = params.bot_id {
        q = q.bind(bot_id);
    }

    if let Some(ref level) = params.level {
        q = q.bind(level);
    }

    let rows = match q.bind(limit).fetch_all(db.as_ref()).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to get activity logs: {}", e);
            return Json(ErrorResponse {
                error: "Failed to get activity logs".to_string(),
            }).into_response();
        }
    };

    #[derive(Serialize)]
    struct LogEntry {
        id: i64,
        bot_id: Option<i64>,
        level: String,
        message: String,
        metadata: Option<String>,
        created_at: String,
    }

    let entries: Vec<LogEntry> = rows.into_iter().map(|row| LogEntry {
        id: row.get("id"),
        bot_id: row.get("bot_id"),
        level: row.get("level"),
        message: row.get("message"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    }).collect();

    Json(entries).into_response()
}

/// Add activity log entry (internal use)
pub async fn log_activity(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let db = state.db();

    let user_id = claims.user_id;
    let bot_id = payload.get("bot_id").and_then(|v| v.as_i64());
    let level = payload.get("level").and_then(|v| v.as_str()).unwrap_or("INFO");
    let message = payload.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let metadata = payload.get("metadata").and_then(|v| v.as_str());

    // Insert into activity_log
    let result = sqlx::query(
        "INSERT INTO activity_log (user_id, bot_id, level, message, metadata) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(bot_id)
    .bind(level)
    .bind(message)
    .bind(metadata)
    .execute(db.as_ref())
    .await;

    match result {
        Ok(_) => Json(serde_json::json!({"success": true})).into_response(),
        Err(e) => {
            tracing::error!("Failed to log activity: {}", e);
            Json(ErrorResponse {
                error: "Failed to log activity".to_string(),
            }).into_response()
        }
    }
}

/// Get bot status with detailed info
#[derive(Debug, Serialize, Deserialize)]
pub struct BotDetailedStatus {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub strategy: String,
    pub market_id: String,
    pub created_at: String,
    pub last_error: Option<String>,
    pub last_activity: Option<String>,
}

pub async fn get_bot_status(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_bot_by_id(&db, id, user_id).await {
        Ok(Some(bot)) => {
            // Get recent logs for this bot
            // For now, return basic info
            Json(BotDetailedStatus {
                id: bot.id,
                name: bot.name,
                status: bot.status,
                strategy: bot.strategy_type,
                market_id: bot.market_id,
                created_at: bot.created_at,
                last_error: None,
                last_activity: None,
            }).into_response()
        }
        Ok(None) => Json(ErrorResponse {
            error: "Bot not found".to_string(),
        }).into_response(),
        Err(e) => {
            tracing::error!("Failed to get bot status: {}", e);
            Json(ErrorResponse {
                error: "Failed to get bot status".to_string(),
            }).into_response()
        }
    }
}

// ==================== Risk Management Endpoints ====================

#[derive(Serialize)]
pub struct RiskStatusResponse {
    pub bot_id: i64,
    pub current_drawdown: f64,
    pub daily_pnl: f64,
    pub trades_today: u32,
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub warnings: Vec<RiskWarningResponse>,
    pub actions: Vec<String>,
}

#[derive(Serialize)]
pub struct RiskWarningResponse {
    pub bot_id: i64,
    pub warning_type: String,
    pub message: String,
    pub severity: String,
    pub timestamp: i64,
}

/// Get risk status for a specific bot
pub async fn get_bot_risk_status(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;

    // Check bot exists
    let db = state.db();
    match queries::get_bot_by_id(&db, id, user_id).await {
        Ok(None) => return Json(ErrorResponse {
            error: "Bot not found".to_string(),
        }).into_response(),
        Err(e) => {
            tracing::error!("Failed to get bot: {}", e);
            return Json(ErrorResponse {
                error: "Failed to get bot".to_string(),
            }).into_response();
        }
        _ => {}
    }

    // Get portfolio for balance info
    let portfolio = match queries::get_portfolio(&db, id, user_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return Json(ErrorResponse {
            error: "No portfolio found for this bot".to_string(),
        }).into_response(),
        Err(_) => return Json(ErrorResponse {
            error: "Failed to get portfolio".to_string(),
        }).into_response(),
    };

    let mut rm = state.orchestrator.risk_manager.write().await;
    let status = rm.get_bot_risk_status(id, portfolio.balance, portfolio.initial_balance);
    let warnings: Vec<RiskWarningResponse> = status.warnings.into_iter().map(|w| RiskWarningResponse {
        bot_id: w.bot_id,
        warning_type: w.warning_type,
        message: w.message,
        severity: w.severity,
        timestamp: w.timestamp,
    }).collect();

    Json(RiskStatusResponse {
        bot_id: id,
        current_drawdown: status.current_drawdown,
        daily_pnl: status.daily_pnl,
        trades_today: status.trades_today,
        paused: status.paused,
        pause_reason: status.pause_reason,
        warnings,
        actions: status.actions,
    }).into_response()
}

/// Pause a bot's risk management
pub async fn pause_bot_risk(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    // Verify bot belongs to user
    match queries::get_bot_by_id(&db, id, claims.user_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "Bot not found".to_string(),
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to verify bot ownership: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to verify bot".to_string(),
            })).into_response();
        }
    }
    let mut rm = state.orchestrator.risk_manager.write().await;
    rm.pause_bot(id, "Manually paused".to_string());
    Json(serde_json::json!({"success": true, "message": "Bot risk paused"})).into_response()
}

/// Resume a bot's risk management
pub async fn resume_bot_risk(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    // Verify bot belongs to user
    match queries::get_bot_by_id(&db, id, claims.user_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "Bot not found".to_string(),
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to verify bot ownership: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to verify bot".to_string(),
            })).into_response();
        }
    }
    let mut rm = state.orchestrator.risk_manager.write().await;
    rm.resume_bot(id);
    Json(serde_json::json!({"success": true, "message": "Bot risk resumed"})).into_response()
}

/// Get all risk warnings
pub async fn get_risk_warnings(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Response {
    let rm = state.orchestrator.risk_manager.read().await;
    let warnings: Vec<RiskWarningResponse> = rm.get_warnings(50).into_iter().map(|w| RiskWarningResponse {
        bot_id: w.bot_id,
        warning_type: w.warning_type,
        message: w.message,
        severity: w.severity,
        timestamp: w.timestamp,
    }).collect();
    Json(warnings).into_response()
}
