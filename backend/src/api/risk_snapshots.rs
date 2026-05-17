//! Risk Snapshots API — persist and load risk metrics for the frontend dashboard
//!
//! Endpoints:
//! - `GET /risk/snapshots` — get latest risk snapshot for each of the user's bots
//! - `POST /risk/snapshots` — save a risk snapshot for a specific bot

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::db::queries;
use crate::middleware::auth::Claims;
use super::AppState;

#[derive(Debug, Serialize)]
pub struct RiskSnapshotResponse {
    pub bot_id: i64,
    pub risk_multiplier: f64,
    pub adjusted_confidence: f64,
    pub kelly_bet: f64,
    pub consecutive_losses: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveRiskSnapshotRequest {
    pub bot_id: i64,
    pub risk_multiplier: f64,
    pub adjusted_confidence: f64,
    pub kelly_bet: f64,
    pub consecutive_losses: i64,
}

/// GET /risk/snapshots — fetch latest risk snapshot for each of the user's bots
pub async fn get_risk_snapshots(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();

    match queries::get_latest_risk_snapshots(&db, claims.user_id).await {
        Ok(snapshots) => {
            let resp: Vec<RiskSnapshotResponse> = snapshots.into_iter().map(|s| RiskSnapshotResponse {
                bot_id: s.bot_id,
                risk_multiplier: s.risk_multiplier,
                adjusted_confidence: s.adjusted_confidence,
                kelly_bet: s.kelly_bet,
                consecutive_losses: s.consecutive_losses,
                created_at: s.created_at,
            }).collect();
            Json(resp).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch risk snapshots: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to fetch risk snapshots"
            }))).into_response()
        }
    }
}

/// POST /risk/snapshots — save a risk snapshot for a bot
pub async fn save_risk_snapshot(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<SaveRiskSnapshotRequest>,
) -> Response {
    let db = state.db();

    match queries::save_risk_snapshot(
        &db,
        payload.bot_id,
        claims.user_id,
        payload.risk_multiplier,
        payload.adjusted_confidence,
        payload.kelly_bet,
        payload.consecutive_losses,
    ).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => {
            tracing::error!("Failed to save risk snapshot: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Failed to save risk snapshot"
            }))).into_response()
        }
    }
}
