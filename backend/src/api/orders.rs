use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::{db::queries, db::OrderRecord, middleware::auth::Claims, trading};
use super::{AppState, CachedCredentials};

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResponse {
    pub id: i64,
    pub bot_id: i64,
    pub market_id: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub status: String,
    pub order_id: Option<String>,
    pub created_at: String,
}

impl From<OrderRecord> for OrderResponse {
    fn from(r: OrderRecord) -> Self {
        Self {
            id: r.id,
            bot_id: r.bot_id,
            market_id: r.market_id,
            side: r.side,
            price: r.price,
            size: r.size,
            status: r.status,
            order_id: r.order_id,
            created_at: r.created_at,
        }
    }
}

pub async fn list_orders(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    if user_id == 0 {
        return Json(ErrorResponse {
            error: "Unauthorized".to_string(),
        })
        .into_response();
    }

    match queries::get_orders_by_user(&db, user_id).await {
        Ok(orders) => Json(orders.into_iter().map(OrderResponse::from).collect::<Vec<_>>()).into_response(),
        Err(e) => {
            tracing::error!("Failed to list orders: {}", e);
            Json(ErrorResponse {
                error: "Failed to list orders".to_string(),
            })
            .into_response()
        }
    }
}

/// Helper: Get decrypted credentials from state (cache or credential service)
async fn get_decrypted_credentials(
    state: &AppState,
    user_id: i64,
) -> Result<CachedCredentials, String> {
    // First try the fast cache (already decrypted credentials)
    {
        let cache = state.credential_cache.read().await;
        if let Some(creds) = cache.get(&user_id) {
            return Ok(creds.clone());
        }
    }

    // Fallback: use credential service to decrypt from database
    let db = state.db();
    match state.credential_service.get_credentials(&db, user_id).await {
        Ok(creds) => {
            // Also populate the fast cache for future requests
            let cached = CachedCredentials {
                api_key: creds.api_key.clone(),
                api_secret: creds.api_secret.clone(),
                api_passphrase: creds.api_passphrase.clone(),
                private_key: creds.private_key.clone(),
                funder: creds.funder.clone(),
                signature_type: creds.signature_type,
                wallet_address: creds.wallet_address.clone(),
            };
            {
                let mut cache = state.credential_cache.write().await;
                cache.insert(user_id, cached.clone());
            }
            Ok(cached)
        }
        Err(e) => Err(format!("Failed to get credentials: {}", e)),
    }
}

fn create_polymarket_client(creds: &CachedCredentials) -> Result<trading::PolymarketClient, String> {
    if creds.private_key.is_empty() {
        return Err("Private key required for trading operations".to_string());
    }

    trading::PolymarketClient::from_api_credentials(
        &creds.private_key,
        creds.signature_type,
        Some(trading::polymarket::ApiKeyCreds {
            key: creds.api_key.clone(),
            secret: creds.api_secret.clone(),
            passphrase: creds.api_passphrase.clone(),
        }),
        creds.funder.as_deref(),
    )
    .map_err(|e| format!("Failed to create client: {}", e))
}

async fn submit_order(
    client: &trading::PolymarketClient,
    order_request: &trading::polymarket::OrderRequest,
    fallback_prefix: &str,
) -> Result<String, String> {
    let signed_order = client
        .create_order(order_request)
        .await
        .map_err(|e| format!("Failed to create order: {}", e))?;

    let response = client
        .post_order(&signed_order)
        .await
        .map_err(|e| format!("Failed to place order: {}", e))?;

    Ok(response.order_id.unwrap_or_else(|| {
        format!("{}_{}", fallback_prefix, chrono::Utc::now().timestamp_millis())
    }))
}

// ============ NEW: Place Order, Cancel, Get Positions ============

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaceOrderRequest {
    pub token_id: String,
    pub price: f64,
    pub size: f64,
    pub side: String, // "BUY" or "SELL"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResult {
    pub success: bool,
    pub order_id: Option<String>,
    pub message: String,
    pub error_code: Option<String>,
}

/// Place an order on Polymarket (checks balance first)
pub async fn place_order(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<PlaceOrderRequest>,
) -> Response {
    let user_id = claims.user_id;

    // Validate side
    let side = payload.side.to_uppercase();
    if side != "BUY" && side != "SELL" {
        return Json(OrderResult {
            success: false,
            order_id: None,
            message: "Side must be BUY or SELL".to_string(),
            error_code: Some("INVALID_SIDE".to_string()),
        })
        .into_response();
    }

    // Validate price and size
    if payload.price <= 0.0 || payload.price >= 1.0 {
        return Json(OrderResult {
            success: false,
            order_id: None,
            message: "Price must be between 0 and 1".to_string(),
            error_code: Some("INVALID_PRICE".to_string()),
        })
        .into_response();
    }

    if payload.size <= 0.0 {
        return Json(OrderResult {
            success: false,
            order_id: None,
            message: "Size must be positive".to_string(),
            error_code: Some("INVALID_SIZE".to_string()),
        })
        .into_response();
    }

    // Get stored credentials (from cache or credential service)
    let creds = match get_decrypted_credentials(&state, user_id).await {
        Ok(c) => c,
        Err(e) => {
            return Json(OrderResult {
                success: false,
                order_id: None,
                message: e,
                error_code: Some("CREDENTIALS_ERROR".to_string()),
            })
            .into_response();
        }
    };

    let pm_client = match create_polymarket_client(&creds) {
        Ok(client) => client,
        Err(message) => {
            let error_code = if message.contains("Private key required") {
                "PRIVATE_KEY_REQUIRED"
            } else {
                "CLIENT_ERROR"
            };

            return Json(OrderResult {
                success: false,
                order_id: None,
                message,
                error_code: Some(error_code.to_string()),
            })
            .into_response();
        }
    };

    // Check balance
    // payload.size is USDC amount from frontend, convert to shares for CLOB
    // CLOB size = number of shares = USDC_amount / price_per_share
    let size_in_shares = payload.size / payload.price;
    let order_value = payload.size; // Order value in USDC is just the amount

    match pm_client.get_balance().await {
        Ok(balance) => {
            tracing::info!(
                "Order check - Balance: {} USDC, Order value: {} USDC, Size: {} shares @ {}",
                balance, order_value, size_in_shares, payload.price
            );

            if balance < order_value {
                return Json(OrderResult {
                    success: false,
                    order_id: None,
                    message: format!(
                        "INSUFFICIENT_BALANCE: Have {} USDC, need {} USDC for this order",
                        balance, order_value
                    ),
                    error_code: Some("INSUFFICIENT_BALANCE".to_string()),
                })
                .into_response();
            }

            // Balance sufficient - place real order on Polymarket
            // CLOB `size` = number of shares, not USDC
            let order_request = trading::polymarket::OrderRequest {
                token_id: payload.token_id.clone(),
                price: payload.price,
                size: size_in_shares,
                side: side.clone(),
            };

            match submit_order(&pm_client, &order_request, "pending").await {
                Ok(order_id) => {
                    tracing::info!(
                        "Real order placed: {} {} @ {} (value: {}) - ID: {}",
                        side, payload.size, payload.price, order_value, order_id
                    );

                    Json(OrderResult {
                        success: true,
                        order_id: Some(order_id),
                        message: format!(
                            "Order placed successfully: {} {} @ {} = {} USDC",
                            side, payload.size, payload.price, order_value
                        ),
                        error_code: None,
                    })
                    .into_response()
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    Json(OrderResult {
                        success: false,
                        order_id: None,
                        message: e,
                        error_code: Some("ORDER_SUBMIT_FAILED".to_string()),
                    })
                    .into_response()
                }
            }
        }
        Err(e) => {
            Json(OrderResult {
                success: false,
                order_id: None,
                message: format!("Failed to check balance: {}", e),
                error_code: Some("BALANCE_CHECK_FAILED".to_string()),
            })
            .into_response()
        }
    }
}

/// Get current positions from Polymarket
pub async fn get_live_positions(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;

    // Get credentials (from cache or credential service)
    let creds = match get_decrypted_credentials(&state, user_id).await {
        Ok(c) => c,
        Err(e) => {
            return Json(ErrorResponse {
                error: e,
            })
            .into_response();
        }
    };

    let pm_client = match create_polymarket_client(&creds) {
        Ok(c) => c,
        Err(message) => {
            return Json(ErrorResponse {
                error: message,
            })
            .into_response();
        }
    };

    match pm_client.get_balance().await {
        Ok(balance) => {
            #[derive(Serialize)]
            struct PositionResponse {
                balance: f64,
                positions: Vec<serde_json::Value>,
            }

            Json(PositionResponse {
                balance,
                positions: vec![],
            })
            .into_response()
        }
        Err(e) => {
            Json(ErrorResponse {
                error: format!("Failed to get positions: {}", e),
            })
            .into_response()
        }
    }
}

/// Cancel an order
#[derive(Debug, Serialize, Deserialize)]
pub struct CancelOrderRequest {
    pub order_id: String,
}

// ============ Quick Trade for UP/DOWN buttons ============

#[derive(Debug, Serialize, Deserialize)]
pub struct QuickTradeRequest {
    /// "UP" or "DOWN" - betting BTC will go above or below beat price
    pub side: String,
    /// Amount in USDC to bet
    pub amount: f64,
    /// Optional market ID - defaults to active BTC 5m market
    pub market_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuickTradeResponse {
    pub success: bool,
    pub message: String,
    pub order_id: Option<String>,
    pub btc_price: Option<f64>,
    pub beat_price: Option<f64>,
    pub side: String,
    pub amount: f64,
    pub error_code: Option<String>,
}

/// Quick trade endpoint for UP/DOWN betting on BTC market
/// UP = bet BTC price will be ABOVE beat_price at end of 5m window
/// DOWN = bet BTC price will be BELOW beat_price at end of 5m window
pub async fn quick_trade(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<QuickTradeRequest>,
) -> Response {
    let user_id = claims.user_id;

    // Validate side
    let side = payload.side.to_uppercase();
    if side != "UP" && side != "DOWN" {
        return Json(QuickTradeResponse {
            success: false,
            message: "Side must be UP or DOWN".to_string(),
            order_id: None,
            btc_price: None,
            beat_price: None,
            side,
            amount: payload.amount,
            error_code: Some("INVALID_SIDE".to_string()),
        })
        .into_response();
    }

    // Validate amount
    if payload.amount <= 0.0 || payload.amount > 1000.0 {
        return Json(QuickTradeResponse {
            success: false,
            message: "Amount must be between 0 and 1000 USDC".to_string(),
            order_id: None,
            btc_price: None,
            beat_price: None,
            side,
            amount: payload.amount,
            error_code: Some("INVALID_AMOUNT".to_string()),
        })
        .into_response();
    }

    // Fetch current BTC price and active market
    let btc_price = match fetch_btc_price_quick().await {
        Some(p) => p,
        None => {
            return Json(QuickTradeResponse {
                success: false,
                message: "Failed to fetch BTC price from Binance".to_string(),
                order_id: None,
                btc_price: None,
                beat_price: None,
                side,
                amount: payload.amount,
                error_code: Some("PRICE_FETCH_FAILED".to_string()),
            })
            .into_response();
        }
    };

    // Fetch active BTC 5m market to get beat_price and token_ids
    let market_info = match fetch_active_btc_market_quick().await {
        Some(m) => m,
        None => {
            return Json(QuickTradeResponse {
                success: false,
                message: "Failed to fetch active BTC market from Polymarket".to_string(),
                order_id: None,
                btc_price: Some(btc_price),
                beat_price: None,
                side,
                amount: payload.amount,
                error_code: Some("MARKET_FETCH_FAILED".to_string()),
            })
            .into_response();
        }
    };

    // Extract beat price
    let beat_price = market_info
        .get("event_metadata")
        .and_then(|m| m.get("price_to_beat"))
        .and_then(|p| p.as_f64())
        .unwrap_or(btc_price);

    // Extract token_ids for YES (UP) and NO (DOWN) outcomes
    let markets = market_info
        .get("markets")
        .and_then(|m| m.as_array())
        .and_then(|arr| arr.first());

    let yes_token_id = markets
        .and_then(|m| m.get("clobTokenIds"))
        .and_then(|ids| ids.as_array())
        .and_then(|arr| arr.first())
        .and_then(|id| id.as_str());

    let no_token_id = markets
        .and_then(|m| m.get("clobTokenIds"))
        .and_then(|ids| ids.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|id| id.as_str());

    let (token_id, outcome) = match side.as_str() {
        "UP" => match yes_token_id {
            Some(token_id) => (token_id, "YES"),
            None => {
                return Json(QuickTradeResponse {
                    success: false,
                    message: "Could not find YES token ID in market data".to_string(),
                    order_id: None,
                    btc_price: Some(btc_price),
                    beat_price: Some(beat_price),
                    side,
                    amount: payload.amount,
                    error_code: Some("TOKEN_ID_MISSING".to_string()),
                })
                .into_response();
            }
        },
        "DOWN" => match no_token_id {
            Some(token_id) => (token_id, "NO"),
            None => {
                return Json(QuickTradeResponse {
                    success: false,
                    message: "Could not find NO token ID in market data".to_string(),
                    order_id: None,
                    btc_price: Some(btc_price),
                    beat_price: Some(beat_price),
                    side,
                    amount: payload.amount,
                    error_code: Some("TOKEN_ID_MISSING".to_string()),
                })
                .into_response();
            }
        },
        _ => unreachable!(),
    };

    // Calculate price based on current probability
    // The market price reflects probability of outcome
    let yes_price = markets
        .and_then(|m| m.get("outcomePrices"))
        .and_then(|p| p.as_str())
        .map(|s| s.split(',').next().unwrap_or("0.5"))
        .and_then(|p| p.parse::<f64>().ok())
        .unwrap_or(0.5);

    let trade_price = if side == "UP" {
        yes_price // Buy YES at current price
    } else {
        1.0 - yes_price // Buy NO (inverse of YES price)
    };

    tracing::info!(
        "Quick trade: {} {} outcome at {} (BTC: {}, Beat: {})",
        side, outcome, trade_price, btc_price, beat_price
    );

    // Get credentials (from cache or credential service)
    let creds = match get_decrypted_credentials(&state, user_id).await {
        Ok(c) => c,
        Err(e) => {
            return Json(QuickTradeResponse {
                success: false,
                message: e,
                order_id: None,
                btc_price: Some(btc_price),
                beat_price: Some(beat_price),
                side,
                amount: payload.amount,
                error_code: Some("CREDENTIALS_ERROR".to_string()),
            })
            .into_response();
        }
    };

    let pm_client = match create_polymarket_client(&creds) {
        Ok(c) => c,
        Err(message) => {
            let error_code = if message.contains("Private key required") {
                "PRIVATE_KEY_REQUIRED"
            } else {
                "CLIENT_ERROR"
            };

            return Json(QuickTradeResponse {
                success: false,
                message,
                order_id: None,
                btc_price: Some(btc_price),
                beat_price: Some(beat_price),
                side,
                amount: payload.amount,
                error_code: Some(error_code.to_string()),
            })
            .into_response();
        }
    };

    // Check balance before placing order
    match pm_client.get_balance().await {
        Ok(balance) => {
            if balance < payload.amount {
                return Json(QuickTradeResponse {
                    success: false,
                    message: format!("INSUFFICIENT_BALANCE: Have {} USDC, need {} USDC", balance, payload.amount),
                    order_id: None,
                    btc_price: Some(btc_price),
                    beat_price: Some(beat_price),
                    side,
                    amount: payload.amount,
                    error_code: Some("INSUFFICIENT_BALANCE".to_string()),
                })
                .into_response();
            }

            // Place the order
            let order_request = trading::polymarket::OrderRequest {
                token_id: token_id.to_string(),
                price: trade_price,
                size: payload.amount / trade_price, // Convert USDC amount to shares
                side: "BUY".to_string(),
            };

            match submit_order(&pm_client, &order_request, "quick").await {
                Ok(order_id) => {
                            tracing::info!(
                                "Quick trade placed: {} {} @ {} = {} USDC - Order: {}",
                                side, outcome, trade_price, payload.amount, order_id
                            );

                            Json(QuickTradeResponse {
                                success: true,
                                message: format!(
                                    "Bet placed: {} outcome at {} ({} USDC)",
                                    outcome, trade_price, payload.amount
                                ),
                                order_id: Some(order_id),
                                btc_price: Some(btc_price),
                                beat_price: Some(beat_price),
                                side,
                                amount: payload.amount,
                                error_code: None,
                            })
                            .into_response()
                }
                Err(e) => {
                    Json(QuickTradeResponse {
                        success: false,
                        message: e,
                        order_id: None,
                        btc_price: Some(btc_price),
                        beat_price: Some(beat_price),
                        side,
                        amount: payload.amount,
                        error_code: Some("ORDER_SUBMIT_FAILED".to_string()),
                    })
                    .into_response()
                }
            }
        }
        Err(e) => {
            Json(QuickTradeResponse {
                success: false,
                message: format!("Failed to check balance: {}", e),
                order_id: None,
                btc_price: Some(btc_price),
                beat_price: Some(beat_price),
                side,
                amount: payload.amount,
                error_code: Some("BALANCE_CHECK_FAILED".to_string()),
            })
            .into_response()
        }
    }
}

/// Fetch BTC price from Binance (for quick trade)
async fn fetch_btc_price_quick() -> Option<f64> {
    let client = reqwest::Client::new();

    match client
        .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            #[derive(Deserialize)]
            struct BinancePrice {
                price: String,
            }

            resp.json::<BinancePrice>().await.ok()?.price.parse::<f64>().ok()
        }
        _ => None,
    }
}

/// Fetch active BTC 5-minute market from Polymarket Gamma API
async fn fetch_active_btc_market_quick() -> Option<serde_json::Value> {
    let client = reqwest::Client::new();

    match client
        .get("https://gamma-api.polymarket.com/events/slug/btc-5")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => resp.json::<serde_json::Value>().await.ok(),
        _ => None,
    }
}

pub async fn cancel_order(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CancelOrderRequest>,
) -> Response {
    let user_id = claims.user_id;

    // Get credentials (from cache or credential service)
    let creds = match get_decrypted_credentials(&state, user_id).await {
        Ok(c) => c,
        Err(e) => {
            return Json(OrderResult {
                success: false,
                order_id: None,
                message: e,
                error_code: Some("CREDENTIALS_ERROR".to_string()),
            })
            .into_response();
        }
    };

    let pm_client = match create_polymarket_client(&creds) {
        Ok(c) => c,
        Err(message) => {
            let error_code = if message.contains("Private key required") {
                "PRIVATE_KEY_REQUIRED"
            } else {
                "CLIENT_ERROR"
            };

            return Json(OrderResult {
                success: false,
                order_id: None,
                message,
                error_code: Some(error_code.to_string()),
            })
            .into_response();
        }
    };

    // Cancel the order on Polymarket
    match pm_client.cancel_order(&payload.order_id).await {
        Ok(_) => {
            tracing::info!("Order cancelled successfully: {}", payload.order_id);

            Json(OrderResult {
                success: true,
                order_id: Some(payload.order_id),
                message: "Order cancelled successfully".to_string(),
                error_code: None,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to cancel order: {}", e);
            Json(OrderResult {
                success: false,
                order_id: None,
                message: format!("Failed to cancel order: {}", e),
                error_code: Some("CANCEL_FAILED".to_string()),
            })
            .into_response()
        }
    }
}
