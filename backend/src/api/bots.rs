use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::db::{queries, BotRecord};
use crate::middleware::auth::Claims;
use crate::trading::PolymarketClient;

fn normalize_mode(mode: &str) -> &'static str {
    match mode {
        "live" => "live",
        _ => "demo",
    }
}

use super::AppState;

// ==================== Response & Request Types ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct BotResponse {
    pub id: i64, pub name: String, pub market_id: String, pub strategy_type: String,
    pub params: String, pub status: String, pub created_at: String, pub bet_size: f64,
    pub use_kelly: bool, pub kelly_fraction: f64, pub max_bet: f64, pub interval: i64,
    pub stop_loss: f64, pub take_profit: f64, pub total_trades: i64, pub winning_trades: i64,
    pub losing_trades: i64, pub win_rate: f64, pub trading_mode: String,
    pub pnl_history: Vec<f64>,
}

impl BotResponse {
    pub async fn from_record_with_history(r: BotRecord, db: &crate::db::Db) -> Self {
        let history = queries::get_recent_decisions(db, r.id, 20).await
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.pnl.unwrap_or(0.0))
            .collect();
            
        Self {
            id: r.id, name: r.name, market_id: r.market_id, strategy_type: r.strategy_type,
            params: r.params, status: r.status, created_at: r.created_at, bet_size: r.bet_size,
            use_kelly: r.use_kelly != 0, kelly_fraction: r.kelly_fraction, max_bet: r.max_bet,
            interval: r.interval, stop_loss: r.stop_loss, take_profit: r.take_profit,
            total_trades: r.total_trades, winning_trades: r.winning_trades, losing_trades: r.losing_trades,
            win_rate: r.win_rate, trading_mode: r.trading_mode,
            pnl_history: history,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse { pub error: String }

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBotRequest {
    pub name: String, pub market_id: String, pub strategy_type: Option<String>,
    pub strategy: Option<String>, pub params: Option<String>, pub bet_size: f64,
    pub use_kelly: bool, pub kelly_fraction: f64, pub max_bet: f64, pub interval: i64,
    pub stop_loss: f64, pub take_profit: f64, pub trading_mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartBotRequest { pub initial_balance: Option<f64>, pub mode: Option<String> }

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBotRequest {
    pub name: Option<String>, pub market_id: Option<String>, pub strategy_type: Option<String>,
    pub params: Option<String>, pub bet_size: Option<f64>, pub use_kelly: Option<bool>,
    pub kelly_fraction: Option<f64>, pub max_bet: Option<f64>, pub interval: Option<i64>,
    pub stop_loss: Option<f64>, pub take_profit: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BotStatusResponse { pub success: bool, pub status: String }

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: i64, pub bot_id: i64, pub status: String, pub start_time: String,
    pub end_time: Option<String>, pub start_balance: f64, pub end_balance: Option<f64>,
    pub total_trades: i64, pub winning_trades: i64, pub losing_trades: i64,
    pub total_pnl: f64, pub max_drawdown: f64,
}

#[derive(Debug, Serialize)]
pub struct PortfolioResponse {
    pub bot_id: i64, pub balance: f64, pub initial_balance: f64, pub open_positions: i64,
    pub total_trades: i64, pub winning_trades: i64, pub losing_trades: i64, pub total_pnl: f64,
    pub peak_balance: f64, pub win_rate: f64, pub roi_percent: f64, pub drawdown_percent: f64,
    pub avg_pnl_per_trade: f64, pub unrealized_pnl: f64, pub total_position_value: f64,
}

#[derive(Debug, Serialize)]
pub struct TradeDecisionResponse {
    pub id: i64, pub bot_id: i64, pub session_id: i64, pub market_slug: String,
    pub outcome: String, pub signal_confidence: f64, pub btc_price: Option<f64>,
    pub yes_price: Option<f64>, pub no_price: Option<f64>, pub time_remaining: Option<i64>,
    pub decision_reason: String, pub created_at: String,
}

impl PortfolioResponse {
    pub fn from_record_with_positions(p: crate::db::BotPortfolioRecord, unrealized_pnl: f64, total_position_value: f64) -> Self {
        let win_rate = if p.total_trades > 0 { p.winning_trades as f64 / p.total_trades as f64 * 100.0 } else { 0.0 };
        Self {
            bot_id: p.bot_id, balance: p.balance, initial_balance: p.initial_balance, open_positions: p.open_positions,
            total_trades: p.total_trades, winning_trades: p.winning_trades, losing_trades: p.losing_trades,
            total_pnl: p.total_pnl, peak_balance: p.peak_balance, win_rate,
            roi_percent: if p.initial_balance > 0.0 { (p.balance - p.initial_balance) / p.initial_balance * 100.0 } else { 0.0 },
            drawdown_percent: if p.peak_balance > 0.0 { (p.peak_balance - p.balance) / p.peak_balance * 100.0 } else { 0.0 },
            avg_pnl_per_trade: if p.total_trades > 0 { p.total_pnl / p.total_trades as f64 } else { 0.0 },
            unrealized_pnl, total_position_value,
        }
    }
}

// ==================== Shared Logic ====================

async fn fetch_unrealized_pnl(state: &AppState, user_id: i64) -> (f64, f64, i64) {
    let cache = state.credential_cache.read().await;
    let creds = cache.get(&user_id).cloned();
    if let Some(c) = creds {
        if let Ok(client) = PolymarketClient::new(&c.private_key) {
            if let Ok(pos) = client.get_positions().await {
                let mut upnl = 0.0; let mut val = 0.0;
                for p in &pos {
                    val += p.current_value.unwrap_or(0.0);
                    upnl += p.current_value.unwrap_or(0.0) - p.total_bought.unwrap_or(0.0);
                }
                return (upnl, val, pos.len() as i64);
            }
        }
    }
    (0.0, 0.0, 0)
}

// ==================== Endpoints ====================

pub async fn create_bot(State(state): State<AppState>, Extension(claims): Extension<Claims>, Json(payload): Json<CreateBotRequest>) -> Response {
    let strategy = payload.strategy_type.or(payload.strategy).unwrap_or_else(|| "momentum".into());
    let params = payload.params.unwrap_or_else(|| "{}".into());
    match queries::create_bot_with_config(
        &state.db(), claims.user_id, &payload.name, &payload.market_id,
        &strategy, &params, payload.bet_size, payload.use_kelly, payload.kelly_fraction,
        payload.max_bet, payload.interval, payload.stop_loss, payload.take_profit,
        &payload.trading_mode
    ).await {
        Ok(id) => {
           let _ = queries::init_portfolio(&state.db(), id, claims.user_id, 100.0).await;
            Json(serde_json::json!({ "id": id, "success": true })).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response()
    }
}


pub async fn list_bots(State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    match queries::get_bots_by_user(&state.db(), claims.user_id).await {
        Ok(bots) => {
            let mut resps = Vec::new();
            for bot in bots {
                resps.push(BotResponse::from_record_with_history(bot, &state.db()).await);
            }
            Json(resps).into_response()
        },
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}

pub async fn get_bot(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    match queries::get_bot_by_id(&state.db(), id, claims.user_id).await {
        Ok(Some(bot)) => Json(BotResponse::from_record_with_history(bot, &state.db()).await).into_response(),
        _ => (StatusCode::NOT_FOUND).into_response()
    }
}

pub async fn start_bot(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    // FIX: Ellenőrzés hogy a bot már fut-e a memóriában
    if state.orchestrator.is_running(id).await {
        return (StatusCode::CONFLICT, Json(ErrorResponse { error: "Bot is already running".to_string() })).into_response();
    }
    if let Ok(Some(bot)) = queries::get_bot_by_id(&state.db(), id, claims.user_id).await {
        // Portfolio egyenleg lekérése (ha nincs, 100.0 az alap)
        let balance = queries::get_portfolio(&state.db(), id, claims.user_id).await
            .ok().flatten().map(|p| p.balance).unwrap_or(100.0);

        // FIX: start_bot az orchestrator-on keresztül → létrehozza a session-t és regisztrálja a bot-ot
        if state.orchestrator.start_bot(&bot, balance).await.is_ok() {
            let orchestrator = state.orchestrator.clone();
            let cred_cache = state.credential_cache.clone();
            let user_id = claims.user_id;
            // FIX: tokio::spawn indítja el a tényleges trading loop-ot
            tokio::spawn(async move {
                crate::trading::orchestrator::start_orchestrator_loop(
                    orchestrator, id, user_id, 15, Some(cred_cache)
                ).await;
            });
            return Json(BotStatusResponse { success: true, status: "running".to_string() }).into_response();
        }
    }
    (StatusCode::INTERNAL_SERVER_ERROR).into_response()
}

pub async fn stop_bot(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    match state.orchestrator.stop_bot(id, claims.user_id).await {
        Ok(_) => Json(BotStatusResponse { success: true, status: "stopped".to_string() }).into_response(),
        _ => (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}

pub async fn get_portfolio(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    match queries::get_portfolio(&state.db(), id, claims.user_id).await {
        Ok(Some(p)) => {
            let (upnl, val, pos_count) = fetch_unrealized_pnl(&state, claims.user_id).await;
            let mut resp = PortfolioResponse::from_record_with_positions(p, upnl, val);
            resp.open_positions = pos_count;

            let has_pending_bet = state.orchestrator.has_pending_bet(id).await;
            if has_pending_bet {
                resp.open_positions = resp.open_positions.max(1);
                if resp.total_position_value == 0.0 {
                    resp.total_position_value = state.orchestrator.get_pending_bet_size(id).await;
                }
            }

            Json(resp).into_response()
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}

pub async fn reset_bot(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    let _ = state.orchestrator.stop_bot(id, claims.user_id).await;
    let _ = queries::reset_portfolio(&state.db(), id, 100.0).await;
    let _ = queries::clear_trade_history(&state.db(), id, claims.user_id).await;
    Json(serde_json::json!({"success": true})).into_response()
}

// --- BULK OPERATIONS ---

pub async fn run_all_bots(State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    if let Ok(bots) = queries::get_bots_by_user(&state.db(), claims.user_id).await {
        for bot in bots {
            if !state.orchestrator.is_running(bot.id).await {
                let balance = queries::get_portfolio(&state.db(), bot.id, claims.user_id).await
                    .ok().flatten().map(|p| p.balance).unwrap_or(100.0);
                let _ = state.orchestrator.start_bot(&bot, balance).await;
                let orchestrator = state.orchestrator.clone();
                let cred_cache = state.credential_cache.clone();
                let bot_id = bot.id;
                let user_id = claims.user_id;
                tokio::spawn(async move {
                    crate::trading::orchestrator::start_orchestrator_loop(
                        orchestrator, bot_id, user_id, 15, Some(cred_cache)
                    ).await;
                });
            }
        }
    }
    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn stop_all_bots(State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    let running = state.orchestrator.get_running_bots(claims.user_id).await;
    for id in running { let _ = state.orchestrator.stop_bot(id, claims.user_id).await; }
    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn reset_all_bots(State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    if let Ok(bots) = queries::get_bots_by_user(&state.db(), claims.user_id).await {
        for bot in bots {
            let _ = state.orchestrator.stop_bot(bot.id, claims.user_id).await;
            let _ = queries::reset_portfolio(&state.db(), bot.id, 100.0).await;
            let _ = queries::clear_trade_history(&state.db(), bot.id, claims.user_id).await;
        }
    }
    Json(serde_json::json!({"success": true})).into_response()
}

// --- Others ---

pub async fn get_session(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(_): Extension<Claims>) -> Response {
    if let Ok(Some(s)) = queries::get_active_session(&state.db(), id).await {
        return Json(SessionResponse {
            session_id: s.id, bot_id: s.bot_id, status: s.status, start_time: s.start_time,
            end_time: s.end_time, start_balance: s.start_balance, end_balance: s.end_balance,
            total_trades: s.total_trades, winning_trades: s.winning_trades, losing_trades: s.losing_trades,
            total_pnl: s.total_pnl, max_drawdown: s.max_drawdown,
        }).into_response();
    }
    (StatusCode::NOT_FOUND).into_response()
}

pub async fn get_trades(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    if let Ok(decisions) = queries::get_trade_decisions(&state.db(), id, claims.user_id).await {
        let resp: Vec<TradeDecisionResponse> = decisions.into_iter().map(|d| TradeDecisionResponse {
            id: d.id, bot_id: d.bot_id, session_id: d.session_id, market_slug: d.market_slug, outcome: d.outcome,
            signal_confidence: d.signal_confidence, btc_price: d.btc_price, yes_price: d.market_yes_price,
            no_price: d.market_no_price, time_remaining: d.time_remaining, decision_reason: d.decision_reason.unwrap_or_default(),
            created_at: d.created_at,
        }).collect();
        return Json(resp).into_response();
    }
    (StatusCode::INTERNAL_SERVER_ERROR).into_response()
}

pub async fn get_history(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    if let Ok(sessions) = queries::get_bot_sessions(&state.db(), id, claims.user_id).await {
        let resp: Vec<SessionResponse> = sessions.into_iter().map(|s| SessionResponse {
            session_id: s.id, bot_id: s.bot_id, status: s.status, start_time: s.start_time,
            end_time: s.end_time, start_balance: s.start_balance, end_balance: s.end_balance,
            total_trades: s.total_trades, winning_trades: s.winning_trades, losing_trades: s.losing_trades,
            total_pnl: s.total_pnl, max_drawdown: s.max_drawdown,
        }).collect();
        return Json(resp).into_response();
    }
    (StatusCode::INTERNAL_SERVER_ERROR).into_response()
}

pub async fn reset_demo_balance(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(_): Extension<Claims>) -> Response {
    let _ = queries::reset_portfolio(&state.db(), id, 10.0).await;
    Json(serde_json::json!({"success": true})).into_response()
}

#[derive(Debug, Deserialize)] pub struct SetModeRequest { pub trading_mode: String }
pub async fn set_all_bots_mode(State(state): State<AppState>, Extension(claims): Extension<Claims>, Json(p): Json<SetModeRequest>) -> Response {
    let mode = if p.trading_mode == "live" { "live" } else { "paper" };
    let _ = sqlx::query("UPDATE bot_configs SET trading_mode = ? WHERE user_id = ?").bind(mode).bind(claims.user_id).execute(state.db().as_ref()).await;
    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn update_bot(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>, Json(p): Json<UpdateBotRequest>) -> Response {
    // 1. Update basic fields (name, market, strategy, params)
    let _ = queries::update_bot(
        &state.db(), id, claims.user_id, 
        p.name.as_deref(), p.market_id.as_deref(), p.strategy_type.as_deref(), p.params.as_deref()
    ).await;
    
    // 2. Update trading configuration fields
    let _ = queries::update_bot_config(
        &state.db(), id, claims.user_id,
        p.bet_size, p.use_kelly, p.kelly_fraction, p.max_bet, p.interval, p.stop_loss, p.take_profit
    ).await;

    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn delete_bot(Path((id,)): Path<(i64,)>, State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    let _ = queries::delete_bot(&state.db(), id, claims.user_id).await;
    Json(serde_json::json!({"success": true})).into_response()
}

pub async fn get_aggregate_portfolio(State(state): State<AppState>, Extension(claims): Extension<Claims>) -> Response {
    let (upnl, val, pos_count) = fetch_unrealized_pnl(&state, claims.user_id).await;
    Json(serde_json::json!({ "unrealized_pnl": upnl, "total_position_value": val, "open_positions": pos_count })).into_response()
}
