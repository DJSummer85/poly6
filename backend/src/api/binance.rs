use axum::{
    extract::State,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;

use crate::trading::btc_price_stream;
use super::AppState;

#[derive(Debug, Serialize)]
pub struct PriceResponse {
    pub connected: bool,
    pub symbol: String,
    pub price: Option<f64>,
}

pub async fn start_binance(
    State(state): State<AppState>,
) -> Response {
    let mut client_lock = state.binance_client.write().await;

    if client_lock.is_some() {
        return Json(serde_json::json!({
            "error": "Binance client already running"
        })).into_response();
    }

    let client = btc_price_stream();
    match client.start().await {
        Ok(_) => {
            *client_lock = Some(client);
            Json(serde_json::json!({
                "connected": true,
                "symbol": "BTCUSDT"
            })).into_response()
        }
        Err(e) => {
            Json(serde_json::json!({
                "error": e
            })).into_response()
        }
    }
}

pub async fn stop_binance(
    State(state): State<AppState>,
) -> Response {
    let mut client_lock = state.binance_client.write().await;

    if let Some(client) = client_lock.take() {
        client.stop().await;
    }

    Json(serde_json::json!({
        "connected": false
    })).into_response()
}

pub async fn get_price(
    State(state): State<AppState>,
) -> Response {
    let client_lock = state.binance_client.read().await;

    match &*client_lock {
        Some(client) => {
            let price = client.get_current_price().await;
            Json(PriceResponse {
                connected: true,
                symbol: "BTCUSDT".to_string(),
                price,
            }).into_response()
        }
        None => {
            Json(PriceResponse {
                connected: false,
                symbol: "BTCUSDT".to_string(),
                price: None,
            }).into_response()
        }
    }
}
