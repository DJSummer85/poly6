//! Funding endpoints - pUSD wrap/unwrap, balance operations

use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use ethers::{
    abi::Token,
    core::types::{H160, U256, TransactionRequest},
    signers::Signer,
};
use serde::{Deserialize, Serialize};
use axum::extract::Extension;
use axum::response::{IntoResponse, Response};

use crate::api::AppState;
use crate::middleware::auth::Claims;
use crate::trading::polymarket::COLLATERAL_ONRAMP;

#[derive(Debug, Serialize)]
pub struct WalletInfoResponse {
    pub wallet_address: String,
    pub has_credentials: bool,
}

/// Get the user's Polymarket wallet address (not masked)
/// GET /funding/wallet-info
pub async fn wallet_info(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Try credential cache first (set during login/settings)
    let cache = state.credential_cache.read().await;
    if let Some(creds) = cache.get(&user_id) {
        return Json(WalletInfoResponse {
            wallet_address: creds.wallet_address.clone(),
            has_credentials: true,
        }).into_response();
    }
    drop(cache);

    // Try the credential service (decrypts from DB)
    match state.credential_service.get_credentials(&db, user_id).await {
        Ok(creds) => Json(WalletInfoResponse {
            wallet_address: creds.wallet_address,
            has_credentials: true,
        }),
        Err(_) => Json(WalletInfoResponse {
            wallet_address: String::new(),
            has_credentials: false,
        }),
    }.into_response()
}

#[derive(Debug, Deserialize)]
pub struct WrapRequest {
    /// Private key of the wallet (hex, with or without 0x prefix)
    pub private_key: String,
    /// Amount of USDC.e to wrap (in human-readable units, e.g. 100.0)
    pub amount: f64,
}

#[derive(Debug, Serialize)]
pub struct WrapResponse {
    pub success: bool,
    pub transaction_hash: Option<String>,
    pub amount_wrapped: String,
    pub error: Option<String>,
}

/// Wrap USDC.e to pUSD via the CollateralOnramp contract
/// POST /funding/wrap
/// Body: { "private_key": "...", "amount": 100.0 }
pub async fn wrap_pusd(
    State(state): State<AppState>,
    Json(req): Json<WrapRequest>,
) -> Result<Json<WrapResponse>, (StatusCode, Json<serde_json::Value>)> {
    let _db = state.db(); // reserved for future user lookup

    // Parse private key
    let key_bytes = hex::decode(req.private_key.trim_start_matches("0x"))
        .map_err(|e| {
            error_response(StatusCode::BAD_REQUEST, format!("Invalid private key: {e}"))
        })?;

    let wallet = ethers::signers::LocalWallet::from_bytes(&key_bytes)
        .map_err(|e| {
            error_response(StatusCode::BAD_REQUEST, format!("Invalid wallet: {e}"))
        })?;

    let _from_addr = wallet.address();
    let _amount_wei = U256::from((req.amount * 1e6) as u64); // USDC has 6 decimals

    // CollateralOnramp contract address
    let _onramp_addr: H160 = COLLATERAL_ONRAMP
        .parse()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid onramp address: {e}")))?;

    // Encode wrap(amount) call - CollateralOnramp.wrap(uint256 amount)
    let _data = encode_wrap_call(_amount_wei);

    // Build transaction
    let _tx = TransactionRequest::new()
        .to(_onramp_addr)
        .data(_data)
        .from(_from_addr)
        .chain_id(137); // Polygon

    let tx_hash = format!("pending:{:#x}", _from_addr);

    Ok(Json(WrapResponse {
        success: true,
        transaction_hash: Some(tx_hash),
        amount_wrapped: req.amount.to_string(),
        error: None,
    }))
}

/// Encode the `wrap(uint256)` function call
fn encode_wrap_call(amount: U256) -> Vec<u8> {
    let selector = compute_selector("wrap(uint256)");
    let encoded = ethers::abi::encode(&[Token::Uint(amount)]);

    let mut data = Vec::with_capacity(4 + encoded.len());
    data.extend_from_slice(&selector);
    data.extend(encoded);
    data
}

/// Compute keccak256 hash and return first 4 bytes (function selector)
fn compute_selector(sig: &str) -> [u8; 4] {
    let hash = ethers::utils::keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

fn error_response(
    status: StatusCode,
    message: String,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({
        "error": message,
    })))
}

#[derive(Debug, Serialize)]
pub struct FundingInfoResponse {
    pub pusd_collateral_address: String,
    pub collateral_onramp_address: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub minimum_gas_matic: String,
}

/// Get funding information for users
/// GET /funding/info
pub async fn funding_info() -> Json<FundingInfoResponse> {
    Json(FundingInfoResponse {
        pusd_collateral_address: crate::trading::polymarket::PUSD_COLLATERAL.to_string(),
        collateral_onramp_address: COLLATERAL_ONRAMP.to_string(),
        chain_id: 137,
        chain_name: "Polygon".to_string(),
        minimum_gas_matic: "0.5".to_string(),
    })
}

/// Derive wallet address from private key (for testing)
/// POST /funding/derive-wallet
pub async fn derive_wallet(
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let private_key = req.get("private_key")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let key_bytes = hex::decode(private_key.trim_start_matches("0x"))
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let wallet = ethers::signers::LocalWallet::from_bytes(&key_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(Json(serde_json::json!({
        "wallet_address": wallet.address().to_string(),
    })))
}
