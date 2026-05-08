use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::db::{queries, PositionRecord};
use crate::middleware::auth::Claims;
use super::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PositionResponse {
    pub id: i64,
    pub bot_id: i64,
    pub market_id: String,
    pub side: String,
    pub size: f64,
    pub avg_price: f64,
    pub current_price: f64,
    pub pnl: f64,
}

impl From<PositionRecord> for PositionResponse {
    fn from(r: PositionRecord) -> Self {
        Self {
            id: r.id,
            bot_id: r.bot_id,
            market_id: r.market_id,
            side: r.side,
            size: r.size,
            avg_price: r.avg_price,
            current_price: r.current_price,
            pnl: r.pnl,
        }
    }
}

pub async fn list_positions(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::get_positions_by_user(&db, user_id).await {
        Ok(positions) => Json(positions.into_iter().map(PositionResponse::from).collect::<Vec<_>>()).into_response(),
        Err(e) => {
            tracing::error!("Failed to list positions: {}", e);
            Json(ErrorResponse {
                error: "Failed to list positions".to_string(),
            })
            .into_response()
        }
    }
}
