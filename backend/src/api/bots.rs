use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::db::{queries, BotRecord};
use crate::middleware::auth::Claims;
use crate::trading::PolymarketClient;

/// Normalize trading mode: "paper" is aliased to "demo" for backwards compatibility.
/// The canonical values are "demo" and "live".
fn normalize_mode(mode: &str) -> &'static str {
    match mode {
        "live" => "live",
        _ => "demo", // "paper" and any other value maps to "demo"
    }
}

fn default_trading_mode() -> String { "demo".to_string() }
use super::AppState;

// ==================== Response Types ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct BotResponse {
    pub id: i64,
    pub name: String,
    pub market_id: String,
    pub strategy_type: String,
    pub params: String,
    pub status: String,
    pub created_at: String,
    pub bet_size: f64,
    pub use_kelly: bool,
    pub kelly_fraction: f64,
    pub max_bet: f64,
    pub interval: i64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub win_rate: f64,
    pub trading_mode: String,
}

impl From<BotRecord> for BotResponse {
    fn from(r: BotRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            market_id: r.market_id,
            strategy_type: r.strategy_type,
            params: r.params,
            status: r.status,
            created_at: r.created_at,
            bet_size: r.bet_size,
            use_kelly: r.use_kelly != 0,
            kelly_fraction: r.kelly_fraction,
            max_bet: r.max_bet,
            interval: r.interval,
            stop_loss: r.stop_loss,
            take_profit: r.take_profit,
            total_trades: r.total_trades,
            winning_trades: r.winning_trades,
            losing_trades: r.losing_trades,
            win_rate: r.win_rate,
            trading_mode: r.trading_mode,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBotRequest {
    pub name: String,
    pub market_id: String,
    pub strategy_type: Option<String>,
    pub strategy: Option<String>,
    pub params: Option<String>,
    #[serde(default = "default_bet_size")]
    pub bet_size: f64,
    #[serde(default = "default_use_kelly")]
    pub use_kelly: bool,
    #[serde(default = "default_kelly_fraction")]
    pub kelly_fraction: f64,
    #[serde(default = "default_max_bet")]
    pub max_bet: f64,
    #[serde(default = "default_interval")]
    pub interval: i64,
    #[serde(default = "default_stop_loss")]
    pub stop_loss: f64,
    #[serde(default = "default_take_profit")]
    pub take_profit: f64,
    #[serde(default = "default_trading_mode")]
    pub trading_mode: String,
}

fn default_bet_size() -> f64 { 1.0 }
fn default_use_kelly() -> bool { true }
fn default_kelly_fraction() -> f64 { 0.25 }
fn default_max_bet() -> f64 { 0.25 }
fn default_interval() -> i64 { 60000 }
fn default_stop_loss() -> f64 { 0.1 }
fn default_take_profit() -> f64 { 0.2 }

#[derive(Debug, Serialize, Deserialize)]
pub struct StartBotRequest {
    pub initial_balance: Option<f64>,
    pub mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBotRequest {
    pub name: Option<String>,
    pub market_id: Option<String>,
    pub strategy_type: Option<String>,
    pub params: Option<String>,
    pub bet_size: Option<f64>,
    pub use_kelly: Option<bool>,
    pub kelly_fraction: Option<f64>,
    pub max_bet: Option<f64>,
    pub interval: Option<i64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BotStatusResponse {
    pub success: bool,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: i64,
    pub bot_id: i64,
    pub status: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub start_balance: f64,
    pub end_balance: Option<f64>,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub total_pnl: f64,
    pub max_drawdown: f64,
}

#[derive(Debug, Serialize)]
pub struct PortfolioResponse {
    pub bot_id: i64,
    pub balance: f64,
    pub initial_balance: f64,
    pub open_positions: i64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub total_pnl: f64,
    pub peak_balance: f64,
    pub win_rate: f64,
    pub roi_percent: f64,
    pub drawdown_percent: f64,
    pub avg_pnl_per_trade: f64,
    pub unrealized_pnl: f64,
    pub total_position_value: f64,
}

impl PortfolioResponse {
    pub fn from_record(p: crate::db::BotPortfolioRecord) -> Self {
        Self::from_record_with_positions(p, 0.0, 0.0)
    }

    pub fn from_record_with_positions(
        p: crate::db::BotPortfolioRecord,
        unrealized_pnl: f64,
        total_position_value: f64,
    ) -> Self {
        let win_rate = if p.total_trades > 0 {
            p.winning_trades as f64 / p.total_trades as f64 * 100.0
        } else {
            0.0
        };

        let roi_percent = if p.initial_balance > 0.0 {
            (p.balance - p.initial_balance) / p.initial_balance * 100.0
        } else {
            0.0
        };

        let drawdown_percent = if p.peak_balance > 0.0 {
            (p.peak_balance - p.balance) / p.peak_balance * 100.0
        } else {
            0.0
        };

        let avg_pnl_per_trade = if p.total_trades > 0 {
            p.total_pnl / p.total_trades as f64
        } else {
            0.0
        };

        Self {
            bot_id: p.bot_id,
            balance: p.balance,
            initial_balance: p.initial_balance,
            open_positions: p.open_positions,
            total_trades: p.total_trades,
            winning_trades: p.winning_trades,
            losing_trades: p.losing_trades,
            total_pnl: p.total_pnl,
            peak_balance: p.peak_balance,
            win_rate,
            roi_percent,
            drawdown_percent,
            avg_pnl_per_trade,
            unrealized_pnl,
            total_position_value,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TradeDecisionResponse {
    pub id: i64,
    pub bot_id: i64,
    pub session_id: i64,
    pub market_slug: String,
    pub outcome: String,
    pub signal_confidence: f64,
    pub btc_price: Option<f64>,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub time_remaining: Option<i64>,
    pub decision_reason: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AggregatePortfolioResponse {
    pub user_id: i64,
    pub total_bots: i64,
    pub running_bots: i64,
    pub total_balance: f64,
    pub total_pnl: f64,
    pub total_trades: i64,
    pub overall_win_rate: f64,
    pub bots: Vec<PortfolioResponse>,
}

// ==================== CRUD Endpoints ====================

pub async fn create_bot(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateBotRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    if payload.name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "Bot name is required".to_string(),
        })).into_response();
    }

    if payload.market_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "Market ID is required".to_string(),
        })).into_response();
    }

    match queries::get_bot_by_name(&db, user_id, &payload.name).await {
        Ok(Some(_)) => {
            return (StatusCode::CONFLICT, Json(ErrorResponse {
                error: format!("Bot with name '{}' already exists", payload.name),
            })).into_response();
        }
        Err(e) => {
            tracing::error!("Failed to check duplicate bot: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to create bot".to_string(),
            })).into_response();
        }
        _ => {}
    }

    let strategy = payload.strategy_type
        .or(payload.strategy)
        .unwrap_or_else(|| "btc_5min".to_string());
    let params = payload.params.unwrap_or_else(|| "{}".to_string());

    match queries::create_bot_with_config(
        &db,
        user_id,
        &payload.name,
        &payload.market_id,
        &strategy,
        &params,
        payload.bet_size,
        payload.use_kelly,
        payload.kelly_fraction,
        payload.max_bet,
        payload.interval,
        payload.stop_loss,
        payload.take_profit,
        &payload.trading_mode,
    ).await {
        Ok(bot_id) => Json(serde_json::json!({
            "id": bot_id,
            "name": payload.name,
            "market_id": payload.market_id,
            "strategy_type": strategy,
            "params": params,
            "status": "stopped",
            "bet_size": payload.bet_size,
            "use_kelly": payload.use_kelly,
            "kelly_fraction": payload.kelly_fraction,
            "max_bet": payload.max_bet,
            "interval": payload.interval,
            "stop_loss": payload.stop_loss,
            "take_profit": payload.take_profit,
            "trading_mode": payload.trading_mode
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to create bot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to create bot".to_string(),
            })).into_response()
        }
    }
}

pub async fn list_bots(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_bots_by_user(&db, user_id).await {
        Ok(bots) => Json(bots.into_iter().map(BotResponse::from).collect::<Vec<_>>()).into_response(),
        Err(e) => {
            tracing::error!("Failed to list bots: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to list bots".to_string(),
            })).into_response()
        }
    }
}

pub async fn get_bot(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_bot_by_id(&db, id, user_id).await {
        Ok(Some(bot)) => Json(BotResponse::from(bot)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "Bot not found".to_string(),
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to get bot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get bot".to_string(),
            })).into_response()
        }
    }
}

pub async fn update_bot(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UpdateBotRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    if let Err(e) = queries::update_bot(
        &db, id, user_id,
        payload.name.as_deref(),
        payload.market_id.as_deref(),
        payload.strategy_type.as_deref(),
        payload.params.as_deref(),
    ).await {
        tracing::error!("Failed to update bot: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "Failed to update bot".to_string(),
        })).into_response();
    }

    if let Err(e) = queries::update_bot_config(
        &db, id, user_id,
        payload.bet_size,
        payload.use_kelly,
        payload.kelly_fraction,
        payload.max_bet,
        payload.interval,
        payload.stop_loss,
        payload.take_profit,
    ).await {
        tracing::error!("Failed to update bot config: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "Failed to update bot config".to_string(),
        })).into_response();
    }

    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn delete_bot(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::delete_bot(&db, id, user_id).await {
        Ok(_) => Json(serde_json::json!({"success": true})).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete bot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to delete bot".to_string(),
            })).into_response()
        }
    }
}

pub async fn start_bot(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    payload: Option<Json<StartBotRequest>>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    let bot = match queries::get_bot_by_id(&db, id, user_id).await {
        Ok(Some(b)) => b,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "Bot not found".to_string(),
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to get bot: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get bot".to_string(),
            })).into_response();
        }
    };

    if state.orchestrator.is_running(id).await {
        return (StatusCode::CONFLICT, Json(ErrorResponse {
            error: "Bot is already running".to_string(),
        })).into_response();
    }

    // Get initial balance and mode from request payload, or use defaults
    let requested_balance = payload
        .as_ref()
        .and_then(|p| p.initial_balance)
        .unwrap_or(10.0);
    let requested_mode = payload.as_ref().and_then(|p| p.mode.as_deref());
    let bot_mode = requested_mode.unwrap_or(&bot.trading_mode);
    let is_demo = normalize_mode(bot_mode) == "demo";
    let initial_balance = if is_demo {
        // Demo mode: use requested balance (default $10) for simulation testing
        let paper_balance = requested_balance;
        tracing::info!("Bot {} running in demo mode — using simulated ${:.2} balance", id, paper_balance);

        let min_required = bot.bet_size.max(1.0);
        if paper_balance < min_required {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: format!("Bet size ${:.2} exceeds paper trading balance ${:.2}", bot.bet_size, paper_balance),
            })).into_response();
        }

        match queries::get_portfolio(&db, id, user_id).await {
            Ok(Some(_)) => {
                if let Err(e) = queries::reset_portfolio(&db, id, paper_balance).await {
                    tracing::warn!("Failed to reset portfolio: {}", e);
                }
            }
            _ => {
                if queries::ensure_portfolio(&db, id, user_id, paper_balance).await.is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error: "Failed to create paper trading portfolio".to_string(),
                    })).into_response();
                }
            }
        }
        paper_balance
    } else {
        // Live trading
        let creds = match state.credential_service.get_credentials(&db, user_id).await {
            Ok(c) => c,
            Err(crate::services::credential_service::CredentialError::NotFound)
            | Err(crate::services::credential_service::CredentialError::PasswordNotCached) => {
                return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                    error: "Cannot start bot: no credentials available. Add API keys in Settings or re-login.".to_string(),
                })).into_response();
            }
            Err(crate::services::credential_service::CredentialError::PrivateKeyRequired) => {
                return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                    error: "Cannot start bot: private key is required for wallet access.".to_string(),
                })).into_response();
            }
            Err(e) => {
                return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                    error: format!("Cannot start bot: credential error: {}", e),
                })).into_response();
            }
        };

        if creds.api_key.is_empty() {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: "Cannot start bot: API key missing. Add your API key in Settings.".to_string(),
            })).into_response();
        }

        let pm_client = match crate::trading::PolymarketClient::new(&creds.private_key) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to create PolymarketClient for balance check: {}", e);
                return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                    error: format!("Failed to create Polymarket client: {}", e),
                })).into_response();
            }
        };

        let wallet_balance = match pm_client.get_balance().await {
            Ok(bal) => bal,
            Err(e) => {
                tracing::warn!("Failed to fetch balance from Polymarket data-api: {}", e);
                return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                    error: format!("Failed to fetch wallet balance: {}", e),
                })).into_response();
            }
        };

        if wallet_balance <= 0.0 {
            return (StatusCode::PAYMENT_REQUIRED, Json(ErrorResponse {
                error: "Insufficient USDC balance. Please deposit USDC to your Polymarket wallet.".to_string(),
            })).into_response();
        }

        tracing::info!("User {} wallet balance: ${:.2}", user_id, wallet_balance);

        let portfolio = match queries::get_portfolio(&db, id, user_id).await {
            Ok(Some(mut p)) => {
                if p.balance != wallet_balance {
                    tracing::info!("Syncing portfolio {} balance from ${:.2} to ${:.2}", p.bot_id, p.balance, wallet_balance);
                    if let Err(e) = queries::update_portfolio_balance(&db, id, wallet_balance).await {
                        tracing::warn!("Failed to update portfolio balance: {}", e);
                    } else {
                        p.balance = wallet_balance;
                    }
                }
                p
            }
            Ok(None) => {
                tracing::info!("Bot {} creating portfolio with wallet balance: ${:.2}", id, wallet_balance);
                match queries::ensure_portfolio(&db, id, user_id, wallet_balance).await {
                    Ok(_) => {
                        if let Ok(Some(p)) = queries::get_portfolio(&db, id, user_id).await {
                            p
                        } else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                                error: "Failed to retrieve created portfolio".to_string(),
                            })).into_response();
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to create portfolio: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                            error: "Failed to create portfolio".to_string(),
                        })).into_response();
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to get portfolio: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: "Failed to get portfolio".to_string(),
                })).into_response();
            }
        };

        let min_required = bot.bet_size.max(1.0);
        if portfolio.balance < min_required {
            return (StatusCode::PAYMENT_REQUIRED, Json(ErrorResponse {
                error: format!("Insufficient balance: need at least ${:.2} to run this bot (bet size: ${:.2})", min_required, bot.bet_size),
            })).into_response();
        }

        let wallet = creds.wallet_address.clone();
        match crate::trading::check_matic_balance(&wallet).await {
            Ok(matic) => tracing::info!("Bot {} MATIC balance check: {:.6}", id, matic),
            Err(e) => tracing::warn!("Bot {} MATIC balance check failed: {}", id, e),
        }

        portfolio.balance
    };

    match state.orchestrator.start_bot(&bot, initial_balance).await {
        Ok(session_id) => {
            tracing::info!("Bot {} started with session {} (balance: {:.2})", id, session_id, initial_balance);

            let interval_secs = (bot.interval / 1000).max(10) as u64;

            let orchestrator = state.orchestrator.clone();
            let cred_cache = state.credential_cache.clone();
            tokio::spawn(async move {
                crate::trading::orchestrator::start_orchestrator_loop(
                    orchestrator, id, user_id, interval_secs, Some(cred_cache)
                ).await;
            });

            Json(BotStatusResponse {
                success: true,
                status: "running".to_string(),
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start bot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: e,
            })).into_response()
        }
    }
}

pub async fn stop_bot(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;

    match state.orchestrator.stop_bot(id, user_id).await {
        Ok(_) => Json(BotStatusResponse {
            success: true,
            status: "stopped".to_string(),
        }).into_response(),
        Err(e) => {
            tracing::error!("Failed to stop bot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: e,
            })).into_response()
        }
    }
}

// ==================== Session Endpoints ====================

pub async fn get_session(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_bot_by_id(&db, id, user_id).await {
        Ok(Some(_)) => {
            match queries::get_active_session(&db, id).await {
                Ok(Some(session)) => Json(SessionResponse {
                    session_id: session.id,
                    bot_id: session.bot_id,
                    status: session.status,
                    start_time: session.start_time,
                    end_time: session.end_time,
                    start_balance: session.start_balance,
                    end_balance: session.end_balance,
                    total_trades: session.total_trades,
                    winning_trades: session.winning_trades,
                    losing_trades: session.losing_trades,
                    total_pnl: session.total_pnl,
                    max_drawdown: session.max_drawdown,
                }).into_response(),
                Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse {
                    error: "No active session".to_string(),
                })).into_response(),
                Err(e) => {
                    tracing::error!("Failed to get session: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error: "Failed to get session".to_string(),
                    })).into_response()
                }
            }
        },
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "Bot not found".to_string(),
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to verify bot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get session".to_string(),
            })).into_response()
        }
    }
}

async fn fetch_unrealized_pnl(state: &AppState, user_id: i64) -> (f64, f64) {
    let cache = state.credential_cache.read().await;
    let creds = cache.get(&user_id).cloned();
    drop(cache);

    let Some(creds) = creds else {
        return (0.0, 0.0);
    };

    let client = match PolymarketClient::new(&creds.private_key) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to create client for position check: {}", e);
            return (0.0, 0.0);
        }
    };

    match client.get_positions().await {
        Ok(positions) => {
            let mut unrealized_pnl = 0.0;
            let mut total_value = 0.0;
            for pos in &positions {
                if let Some(value) = pos.current_value {
                    total_value += value;
                    if let Some(bought) = pos.total_bought {
                        unrealized_pnl += value - bought;
                    }
                }
            }
            (unrealized_pnl, total_value)
        }
        Err(e) => {
            tracing::warn!("Failed to fetch positions for user {}: {}", user_id, e);
            (0.0, 0.0)
        }
    }
}

pub async fn get_portfolio(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Auto-create portfolio with default balance if none exists (for bots that haven't been started yet)
    let portfolio = match queries::get_portfolio(&db, id, user_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            // Create a default paper portfolio for bots without one
            if let Err(e) = queries::ensure_portfolio(&db, id, user_id, 100.0).await {
                tracing::error!("Failed to create portfolio for bot {}: {}", id, e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: "Failed to create portfolio".to_string(),
                })).into_response();
            }
            match queries::get_portfolio(&db, id, user_id).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    return (StatusCode::NOT_FOUND, Json(ErrorResponse {
                        error: "No portfolio found for this bot".to_string(),
                    })).into_response();
                }
                Err(e) => {
                    tracing::error!("Failed to get portfolio after creation: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                        error: "Failed to get portfolio".to_string(),
                    })).into_response();
                }
            }
        },
        Err(e) => {
            tracing::error!("Failed to get portfolio: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get portfolio".to_string(),
            })).into_response();
        }
    };

    let (unrealized_pnl, total_position_value) = fetch_unrealized_pnl(&state, user_id).await;

    Json(PortfolioResponse::from_record_with_positions(
        portfolio, unrealized_pnl, total_position_value,
    )).into_response()
}

pub async fn get_history(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_bot_sessions(&db, id, user_id).await {
        Ok(sessions) => {
            let response: Vec<SessionResponse> = sessions.into_iter().map(|s| SessionResponse {
                session_id: s.id,
                bot_id: s.bot_id,
                status: s.status,
                start_time: s.start_time,
                end_time: s.end_time,
                start_balance: s.start_balance,
                end_balance: s.end_balance,
                total_trades: s.total_trades,
                winning_trades: s.winning_trades,
                losing_trades: s.losing_trades,
                total_pnl: s.total_pnl,
                max_drawdown: s.max_drawdown,
            }).collect();
            Json(response).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to get history: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get history".to_string(),
            })).into_response()
        }
    }
}

pub async fn get_trades(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_trade_decisions(&db, id, user_id).await {
        Ok(decisions) => {
            let response: Vec<TradeDecisionResponse> = decisions.into_iter().map(|d| TradeDecisionResponse {
                id: d.id,
                bot_id: d.bot_id,
                session_id: d.session_id,
                market_slug: d.market_slug,
                outcome: d.outcome,
                signal_confidence: d.signal_confidence,
                btc_price: d.btc_price,
                yes_price: d.market_yes_price,
                no_price: d.market_no_price,
                time_remaining: d.time_remaining,
                decision_reason: d.decision_reason.unwrap_or_default(),
                created_at: d.created_at,
            }).collect();
            Json(response).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to get trades: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get trades".to_string(),
            })).into_response()
        }
    }
}

// ==================== Bulk Operations ====================

pub async fn run_all_bots(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    let cache = state.credential_cache.read().await;
    let creds = cache.get(&user_id).cloned();
    drop(cache);

    let (wallet_balance, has_credentials) = match &creds {
        Some(c) if !c.private_key.is_empty() => {
            if let Ok(pm_client) = crate::trading::PolymarketClient::new(&c.private_key) {
                match pm_client.get_balance().await {
                    Ok(bal) => (bal, true),
                    Err(e) => {
                        tracing::warn!("Failed to fetch wallet balance for run_all: {}", e);
                        (0.0, false)
                    }
                }
            } else {
                (0.0, false)
            }
        }
        _ => (0.0, false),
    };

    match queries::get_bots_by_user(&db, user_id).await {
        Ok(bots) => {
            let total = bots.len();
            let mut started = 0;
            let mut skipped_zero_balance = 0;
            let mut skipped_no_creds = 0;

            for bot in bots {
                if state.orchestrator.is_running(bot.id).await {
                    continue;
                }

                let is_demo = normalize_mode(&bot.trading_mode) == "demo";
                let balance_to_use = if is_demo {
                    100.0 // Default paper balance
                } else {
                    wallet_balance
                };

                if !is_demo && balance_to_use <= 0.0 {
                    if has_credentials {
                        skipped_zero_balance += 1;
                    } else {
                        skipped_no_creds += 1;
                    }
                    continue;
                }

                // Ensure portfolio exists
                match queries::get_portfolio(&db, bot.id, user_id).await {
                    Ok(None) => {
                        if let Err(e) = queries::ensure_portfolio(&db, bot.id, user_id, balance_to_use).await {
                            tracing::error!("Failed to create portfolio for bot {}: {}", bot.id, e);
                            continue;
                        }
                    }
                    Ok(Some(_)) => {}
                    Err(e) => {
                        tracing::error!("Failed to get portfolio for bot {}: {}", bot.id, e);
                        continue;
                    }
                }

                if state.orchestrator.start_bot(&bot, balance_to_use).await.is_ok() {
                    let interval_secs = (bot.interval / 1000).max(10) as u64;
                    let orchestrator = state.orchestrator.clone();
                    let cred_cache = state.credential_cache.clone();
                    let bot_id = bot.id;
                    tokio::spawn(async move {
                        crate::trading::orchestrator::start_orchestrator_loop(
                            orchestrator, bot_id, user_id, interval_secs, Some(cred_cache)
                        ).await;
                    });
                    started += 1;
                }
            }

            Json(serde_json::json!({
                "success": true,
                "started": started,
                "total": total,
                "skipped_zero_balance": skipped_zero_balance,
                "skipped_no_creds": skipped_no_creds
            })).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to run all bots: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to run all bots".to_string(),
            })).into_response()
        }
    }
}

pub async fn stop_all_bots(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;

    let running = state.orchestrator.get_running_bots(user_id).await;
    let mut stopped = 0;

    for bot_id in running {
        if state.orchestrator.stop_bot(bot_id, user_id).await.is_ok() {
            stopped += 1;
        }
    }

    Json(serde_json::json!({
        "success": true,
        "stopped": stopped
    })).into_response()
}

pub async fn reset_demo_balance(
    Path((id,)): Path<(i64,)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Verify bot belongs to user
    match queries::get_bot_by_id(&db, id, user_id).await {
        Ok(Some(bot)) => {
            if normalize_mode(&bot.trading_mode) == "live" {
                return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                    error: "Only demo/paper bots can be reset".to_string(),
                })).into_response();
            }
        }
        Ok(None) => return (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "Bot not found".to_string(),
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to get bot for reset: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to verify bot".to_string(),
            })).into_response();
        }
    }

    // Reset portfolio to $10
    match queries::reset_portfolio(&db, id, 10.0).await {
        Ok(_) => {
            tracing::info!("Demo bot {} reset to $10", id);
            Json(serde_json::json!({
                "success": true,
                "message": "Demo balance reset to $10",
                "new_balance": 10.0
            })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to reset demo portfolio: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to reset demo balance".to_string(),
            })).into_response()
        }
    }
}

pub async fn get_aggregate_portfolio(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    let (unrealized_pnl, total_position_value) = fetch_unrealized_pnl(&state, user_id).await;

    match queries::get_bots_by_user(&db, user_id).await {
        Ok(bots) => {
            let mut total_balance = 0.0;
            let mut total_initial = 0.0;
            let mut total_pnl = 0.0;
            let mut total_trades = 0;
            let mut total_wins = 0;
            let mut running_bots = 0;
            let mut bot_portfolios = Vec::new();

            for bot in &bots {
                if let Ok(Some(portfolio)) = queries::get_portfolio(&db, bot.id, user_id).await {
                    total_balance += portfolio.balance;
                    total_initial += portfolio.initial_balance;
                    total_pnl += portfolio.total_pnl;
                    total_trades += portfolio.total_trades;
                    total_wins += portfolio.winning_trades;

                    bot_portfolios.push(PortfolioResponse::from_record_with_positions(
                        portfolio, 0.0, 0.0,
                    ));
                }

                if state.orchestrator.is_running(bot.id).await {
                    running_bots += 1;
                }
            }

            if total_trades == 0 {
                return Json(serde_json::json!({
                    "total_bots": bots.len(),
                    "running_bots": running_bots,
                    "total_balance": total_balance,
                    "total_initial": total_initial,
                    "total_pnl": 0.0,
                    "total_trades": 0,
                    "overall_win_rate": 0.0,
                    "overall_roi_percent": 0.0,
                    "avg_pnl_per_trade": 0.0,
                    "unrealized_pnl": 0.0,
                    "total_position_value": 0.0,
                    "bots": bot_portfolios
                })).into_response();
            }

            let overall_win_rate = if total_trades > 0 {
                total_wins as f64 / total_trades as f64 * 100.0
            } else {
                0.0
            };

            let overall_roi = if total_initial > 0.0 {
                (total_balance - total_initial) / total_initial * 100.0
            } else {
                0.0
            };

            let avg_pnl_per_trade = if total_trades > 0 {
                total_pnl / total_trades as f64
            } else {
                0.0
            };

            Json(serde_json::json!({
                "user_id": user_id,
                "total_bots": bots.len(),
                "running_bots": running_bots,
                "total_balance": total_balance,
                "total_initial": total_initial,
                "total_pnl": total_pnl,
                "total_trades": total_trades,
                "overall_win_rate": overall_win_rate,
                "overall_roi_percent": overall_roi,
                "avg_pnl_per_trade": avg_pnl_per_trade,
                "unrealized_pnl": unrealized_pnl,
                "total_position_value": total_position_value,
                "bots": bot_portfolios
            })).into_response()
        },
        Err(e) => {
            tracing::error!("Failed to get aggregate portfolio: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to get portfolio".to_string(),
            })).into_response()
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct SetModeRequest {
    pub trading_mode: String,
}

pub async fn set_all_bots_mode(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<SetModeRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    let mode = match payload.trading_mode.as_str() {
        "live" => "live",
        _ => "paper",
    };

    match sqlx::query(
        "UPDATE bot_configs SET trading_mode = ?, updated_at = datetime('now') WHERE user_id = ?"
    )
    .bind(mode)
    .bind(user_id)
    .execute(db.as_ref())
    .await {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "trading_mode": mode
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to set bots mode: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "Failed to update bots mode".to_string(),
            })).into_response()
        }
    }
}