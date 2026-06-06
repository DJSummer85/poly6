//! Funding endpoints - pUSD wrap/unwrap, balance operations
//!
//! Provides on-chain wrapping of USDC.e → pUSD via the CollateralOnramp contract.
//! Uses raw JSON-RPC calls (like the rest of the codebase) for broadcasting transactions.

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
use std::time::Duration;

use crate::api::AppState;
use crate::middleware::auth::Claims;
use crate::trading::polymarket::{check_matic_balance, COLLATERAL_ONRAMP, USDC_E_TOKEN, try_polygon_rpcs};

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
        Ok(creds) => {
            // Load deposit wallet address from DB (separate from credential service)
            let deposit_wallet_address = if let Ok(keys) = crate::db::queries::get_api_keys(&db, user_id).await {
                keys.iter()
                    .find(|k| k.key_name == "polymarket_deposit_wallet_address")
                    .map(|k| k.key_value.clone())
            } else {
                None
            };

            let mut cache = state.credential_cache.write().await;
            cache.insert(user_id, crate::api::CachedCredentials {
                api_key: creds.api_key,
                api_secret: creds.api_secret,
                api_passphrase: creds.api_passphrase,
                private_key: creds.private_key,
                funder: creds.funder,
                signature_type: creds.signature_type,
                wallet_address: creds.wallet_address.clone(),
                deposit_wallet_address,
            });
            Json(WalletInfoResponse {
                wallet_address: creds.wallet_address,
                has_credentials: true,
            })
        },
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
    pub approval_tx_hash: Option<String>,
    pub error: Option<String>,
}

/// Helper: call Polygon RPC with automatic fallback across multiple endpoints.
async fn call_rpc(body: &serde_json::Value) -> Result<serde_json::Value, String> {
    let (rpc_url, result) = try_polygon_rpcs(body).await?;
    // Check for RPC error in the response itself
    if let Some(err) = result.get("error") {
        return Err(format!("RPC error from {}: {}", rpc_url, err));
    }
    Ok(result)
}

/// Wait for a transaction to be mined by polling eth_getTransactionReceipt.
/// Polls every 1.5 seconds, times out after 60 seconds.
async fn wait_for_tx_receipt(tx_hash: &str) -> Result<(), String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(60);

    loop {
        if start.elapsed() > timeout {
            return Err(format!("Timeout waiting for tx {} after 60s", tx_hash));
        }

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });

        let result = call_rpc(&body).await
            .map_err(|e| format!("RPC receipt error: {e}"))?;

        let receipt = result.get("result");
        if receipt.is_some() && !receipt.unwrap().is_null() {
            // Check for failure
            if let Some(status) = receipt.unwrap().get("status").and_then(|s| s.as_str()) {
                if status == "0x0" {
                    return Err(format!("Transaction {} failed (status=0x0)", tx_hash));
                }
            }
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
}

/// Send a raw signed transaction via Polygon RPC (eth_sendRawTransaction).
/// Returns the transaction hash on success.
async fn send_raw_tx(signed_bytes: &[u8]) -> Result<String, String> {
    let hex_tx = format!("0x{}", hex::encode(signed_bytes));

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": [hex_tx],
        "id": 1
    });

    let result = call_rpc(&body).await
        .map_err(|e| format!("RPC eth_sendRawTransaction error: {e}"))?;

    result
        .get("result")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No tx hash in response".to_string())
}

/// RLP encode a byte array (string).
fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        // Single byte < 0x80: just the byte
        return vec![data[0]];
    }
    if data.len() <= 55 {
        // 0x80 + length + bytes
        let mut out = Vec::with_capacity(1 + data.len());
        out.push(0x80 + data.len() as u8);
        out.extend_from_slice(data);
        return out;
    }
    // Longer string: 0xb7 + length_of_length + length + bytes
    let len_str = format!("{:x}", data.len());
    let len_bytes = hex::decode(&len_str).unwrap_or_else(|_| vec![data.len() as u8]);
    let mut out = Vec::with_capacity(1 + len_bytes.len() + data.len());
    out.push(0xb7 + len_bytes.len() as u8);
    out.extend_from_slice(&len_bytes);
    out.extend_from_slice(data);
    out
}

/// RLP encode a u64 integer.
fn rlp_encode_u64(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80]; // empty string = 0
    }
    let hex_str = format!("{:x}", val);
    let bytes = if hex_str.len() % 2 == 0 {
        hex::decode(&hex_str).unwrap()
    } else {
        hex::decode(format!("0{}", hex_str)).unwrap()
    };
    rlp_encode_bytes(&bytes)
}

/// RLP encode a U256 integer.
fn rlp_encode_u256(val: &U256) -> Vec<u8> {
    if val.is_zero() {
        return vec![0x80];
    }
    let mut be_bytes = vec![0u8; 32];
    val.to_big_endian(&mut be_bytes);
    // Trim leading zeros
    let trimmed: Vec<u8> = be_bytes.into_iter().skip_while(|&b| b == 0).collect();
    rlp_encode_bytes(&trimmed)
}

/// RLP encode a list of already-encoded items.
fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let mut flat = Vec::new();
    for item in items {
        flat.extend_from_slice(item);
    }
    if flat.len() <= 55 {
        let mut out = Vec::with_capacity(1 + flat.len());
        out.push(0xc0 + flat.len() as u8);
        out.extend_from_slice(&flat);
        return out;
    }
    let len_str = format!("{:x}", flat.len());
    let len_bytes = hex::decode(&len_str).unwrap_or_else(|_| vec![flat.len() as u8]);
    let mut out = Vec::with_capacity(1 + len_bytes.len() + flat.len());
    out.push(0xf7 + len_bytes.len() as u8);
    out.extend_from_slice(&len_bytes);
    out.extend_from_slice(&flat);
    out
}

/// Build, sign, and send a legacy Ethereum transaction via RPC.
/// Uses the wallet to sign, then RLP-encodes with EIP-155 replay protection (chain_id = 137).
async fn sign_and_send_tx(
    wallet: &ethers::signers::LocalWallet,
    to: H160,
    data: Vec<u8>,
    value: U256,
    gas_limit_override: Option<u64>,
) -> Result<String, String> {
    let from_addr = wallet.address();
    let chain_id = 137u64; // Polygon

    // 1) Nonce
    let nonce_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [format!("{:#042x}", from_addr), "pending"],
        "id": 1
    });
    let nonce_val = call_rpc(&nonce_body).await
        .map_err(|e| format!("RPC nonce error: {e}"))?;
    let nonce_hex = nonce_val
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid nonce response".to_string())?;
    let nonce = u64::from_str_radix(nonce_hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Bad nonce hex: {e}"))?;

    // 2) Gas price
    let gp_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_gasPrice",
        "params": [],
        "id": 1
    });
    let gp_json = call_rpc(&gp_body).await
        .map_err(|e| format!("RPC gasPrice error: {e}"))?;
    let gp_hex = gp_json
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid gasPrice response".to_string())?;
    let gas_price_u128 = u128::from_str_radix(gp_hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Bad gasPrice hex: {e}"))?;

    // 3) Gas limit (use override or estimate)
    let gas_limit = if let Some(override_limit) = gas_limit_override {
        override_limit
    } else {
        let est_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [{
                "from": format!("{:#042x}", from_addr),
                "to": format!("{:#06x}", to),
                "data": format!("0x{}", hex::encode(&data)),
                "value": format!("{:#x}", value)
            }],
            "id": 1
        });
        let est_json = call_rpc(&est_body).await
            .map_err(|e| format!("RPC estimateGas error: {e}"))?;
        let est_hex = est_json
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Invalid estimateGas response".to_string())?;
        let estimated = u64::from_str_radix(est_hex.trim_start_matches("0x"), 16)
            .map_err(|e| format!("Bad estimateGas hex: {e}"))?;
        // Add 20% buffer for safety
        (estimated as f64 * 1.2) as u64
    };

    // 4) Build TransactionRequest
    let tx = TransactionRequest::new()
        .to(to)
        .data(data.clone())
        .value(value)
        .nonce(nonce)
        .gas_price(gas_price_u128)
        .gas(gas_limit)
        .chain_id(chain_id)
        .from(from_addr);

    // 5) Sign (ethers handles EIP-155 v internally)
    let signature = wallet.sign_transaction(&tx.clone().into())
        .await
        .map_err(|e| format!("Signing failed: {e}"))?;

    let sig_r: [u8; 32] = signature.r.into();
    let sig_s: [u8; 32] = signature.s.into();
    let sig_v = signature.v as u64;

    // 6) RLP encode: [nonce, gasPrice, gasLimit, to, value, data, v, r, s]
    let to_bytes = to.as_bytes().to_vec();
    let encoded = rlp_encode_list(&[
        rlp_encode_u64(nonce),
        rlp_encode_u256(&U256::from(gas_price_u128)),
        rlp_encode_u64(gas_limit),
        rlp_encode_bytes(&to_bytes),
        rlp_encode_u256(&value),
        rlp_encode_bytes(&data),
        rlp_encode_u64(sig_v),
        rlp_encode_bytes(&sig_r),
        rlp_encode_bytes(&sig_s),
    ]);

    // 7) Send
    send_raw_tx(&encoded).await
}

/// Encode the ERC20 `approve(address spender, uint256 amount)` function call.
fn encode_approve_call(spender: H160, amount: U256) -> Vec<u8> {
    let selector = compute_selector("approve(address,uint256)");
    let encoded = ethers::abi::encode(&[
        Token::Address(spender),
        Token::Uint(amount),
    ]);
    let mut data = Vec::with_capacity(4 + encoded.len());
    data.extend_from_slice(&selector);
    data.extend(encoded);
    data
}

/// Wrap USDC.e to pUSD via the CollateralOnramp contract
///
/// 1. Approve CollateralOnramp to spend USDC.e (if not already approved)
/// 2. Call CollateralOnramp.wrap(amount) to mint pUSD
///
/// POST /funding/wrap
/// Body: { "private_key": "...", "amount": 100.0 }
pub async fn wrap_pusd(
    State(state): State<AppState>,
    Json(req): Json<WrapRequest>,
) -> Result<Json<WrapResponse>, (StatusCode, Json<serde_json::Value>)> {
    let _db = state.db();

    // Parse private key
    let key_bytes = hex::decode(req.private_key.trim_start_matches("0x"))
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, format!("Invalid private key: {e}")))?;

    let wallet = ethers::signers::LocalWallet::from_bytes(&key_bytes)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, format!("Invalid wallet: {e}")))?;

    let from_addr = wallet.address();
    let amount_wei = U256::from((req.amount * 1_000_000.0) as u64); // USDC has 6 decimals

    let onramp_addr: H160 = COLLATERAL_ONRAMP
        .parse()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid onramp address: {e}")))?;

    let usdc_e_addr: H160 = USDC_E_TOKEN
        .parse()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid USDC.e address: {e}")))?;

    // Step 1: Check if we need to approve
    // Encode allowance call to check current approval
    // allowance(address owner, address spender)
    let allowance_selector = compute_selector("allowance(address,address)");
    let allowance_data = {
        let encoded = ethers::abi::encode(&[
            Token::Address(from_addr),
            Token::Address(onramp_addr),
        ]);
        let mut data = Vec::with_capacity(4 + encoded.len());
        data.extend_from_slice(&allowance_selector);
        data.extend(encoded);
        data
    };

    // Query current allowance via eth_call
    let allowance_call_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{
            "to": format!("{:#042x}", usdc_e_addr),
            "data": format!("0x{}", hex::encode(&allowance_data))
        }, "latest"],
        "id": 1
    });

    let allowance_json = call_rpc(&allowance_call_body).await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("RPC allowance error: {e}")))?;

    let allowance_hex = allowance_json
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Invalid allowance response".to_string()))?;

    let current_allowance = if allowance_hex == "0x" || allowance_hex.len() < 3 {
        U256::zero()
    } else {
        let raw = u128::from_str_radix(allowance_hex.trim_start_matches("0x"), 16)
            .unwrap_or(0);
        U256::from(raw)
    };

    let mut approval_tx_hash: Option<String> = None;

    // Step 2: Approve if needed
    if current_allowance < amount_wei {
        tracing::info!("Approving {:.2} USDC.e for onramp contract...", req.amount);
        let approve_data = encode_approve_call(onramp_addr, amount_wei);
        approval_tx_hash = Some(
            sign_and_send_tx(&wallet, usdc_e_addr, approve_data, U256::zero(), Some(200_000))
                .await
                .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Approval tx failed: {e}")))?,
        );
        tracing::info!("Approval sent: {:?}", approval_tx_hash);

        // Wait for the approval to be mined before sending the wrap
        if let Some(ref hash) = approval_tx_hash {
            wait_for_tx_receipt(hash).await
                .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Approval not confirmed: {e}")))?;
        }
    }

    // Step 3: Wrap
    tracing::info!("Wrapping {:.2} USDC.e → pUSD...", req.amount);
    let wrap_data = encode_wrap_call(amount_wei);
    let wrap_tx_hash = sign_and_send_tx(&wallet, onramp_addr, wrap_data, U256::zero(), None)
        .await
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Wrap tx failed: {e}")))?;

    tracing::info!("Wrap sent: {}", wrap_tx_hash);

    Ok(Json(WrapResponse {
        success: true,
        transaction_hash: Some(wrap_tx_hash),
        amount_wrapped: req.amount.to_string(),
        approval_tx_hash,
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

#[derive(Debug, Serialize)]
pub struct MaticBalanceResponse {
    pub wallet_address: String,
    pub balance_matic: f64,
    pub has_sufficient_gas: bool,
    pub minimum_recommended: f64,
}

/// Get MATIC gas balance for the authenticated user's wallet
/// GET /funding/matic-balance
pub async fn matic_balance(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Get wallet address from cache or credential service
    let wallet_address = {
        let cache = state.credential_cache.read().await;
        if let Some(creds) = cache.get(&user_id) {
            if !creds.wallet_address.is_empty() {
                Some(creds.wallet_address.clone())
            } else {
                None
            }
        } else {
            None
        }
    };

    let wallet_address = if let Some(addr) = wallet_address {
        addr
    } else {
        // Try credential service
        match state.credential_service.get_credentials(&db, user_id).await {
            Ok(creds) => {
                let addr = creds.wallet_address.clone();
                let mut cache = state.credential_cache.write().await;
                cache.insert(user_id, crate::api::CachedCredentials {
                    api_key: creds.api_key,
                    api_secret: creds.api_secret,
                    api_passphrase: creds.api_passphrase,
                    private_key: creds.private_key,
                    funder: creds.funder,
                    signature_type: creds.signature_type,
                    wallet_address: addr.clone(),
                    deposit_wallet_address: None,
                });
                addr
            }
            Err(_) => {
                return Json(serde_json::json!({
                    "error": "No credentials found. Set up wallet credentials in Settings first.",
                    "wallet_address": "",
                    "balance_matic": 0.0,
                    "has_sufficient_gas": false,
                    "minimum_recommended": 0.5,
                })).into_response();
            }
        }
    };

    match check_matic_balance(&wallet_address).await {
        Ok(balance) => {
            let has_sufficient = balance >= 0.5; // 0.5 MATIC recommended minimum
            Json(MaticBalanceResponse {
                wallet_address,
                balance_matic: balance,
                has_sufficient_gas: has_sufficient,
                minimum_recommended: 0.5,
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to check MATIC balance: {}", e);
            Json(MaticBalanceResponse {
                wallet_address,
                balance_matic: 0.0,
                has_sufficient_gas: false,
                minimum_recommended: 0.5,
            }).into_response()
        }
    }
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

#[derive(Debug, Serialize)]
pub struct DepositWalletResponse {
    pub success: bool,
    pub wallet_address: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveDepositWalletRequest {
    /// The deposit wallet address from Polymarket.com (0x-prefixed hex)
    pub wallet_address: String,
}

/// Save a deposit wallet address for the authenticated user.
/// The address must be obtained from Polymarket.com Settings -> API Keys.
/// POST /funding/save-deposit-wallet
pub async fn save_deposit_wallet(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SaveDepositWalletRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;
    let addr = req.wallet_address.trim().to_string();

    // Basic validation
    if !addr.starts_with("0x") || addr.len() != 42 {
        return Json(DepositWalletResponse {
            success: false,
            wallet_address: None,
            message: "Invalid address format. Must be 0x-prefixed hex (42 chars total).".to_string(),
        }).into_response();
    }

    // Store in credential cache
    {
        let mut cache = state.credential_cache.write().await;
        if let Some(creds) = cache.get_mut(&user_id) {
            creds.deposit_wallet_address = Some(addr.clone());
        } else {
            // Create minimal cache entry if doesn't exist
            cache.insert(user_id, crate::api::CachedCredentials {
                api_key: String::new(),
                api_secret: String::new(),
                api_passphrase: String::new(),
                private_key: String::new(),
                funder: None,
                signature_type: 3,
                wallet_address: String::new(),
                deposit_wallet_address: Some(addr.clone()),
            });
        }
        tracing::info!("Stored deposit wallet address {} in cache for user {}", addr, user_id);
    }

    // Store in DB api_keys table
    let pool = db.as_ref();
    let _ = sqlx::query(
        "INSERT OR REPLACE INTO api_keys (user_id, key_name, key_value) VALUES (?, ?, ?)"
    )
    .bind(user_id)
    .bind("polymarket_deposit_wallet_address")
    .bind(&addr)
    .execute(pool)
    .await;

    // Optionally check if the address has code on-chain
    let deployed = crate::trading::polymarket::PolymarketClient::check_deposit_wallet_exists(&addr)
        .await
        .unwrap_or(false);

    Json(DepositWalletResponse {
        success: true,
        wallet_address: Some(addr),
        message: if deployed {
            "Deposit wallet address saved and confirmed deployed on-chain.".to_string()
        } else {
            "Deposit wallet address saved. Note: not yet visible on-chain — it may be deployed when you place your first order.".to_string()
        },
    }).into_response()
}

/// Auto-fetch the deposit wallet address from the Polymarket relayer API.
/// Uses Gamma auth (same CLOB credentials) to query the relayer.
/// POST /funding/fetch-deposit-wallet
pub async fn fetch_deposit_wallet(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Get credentials from cache
    let cache = state.credential_cache.read().await;
    let (private_key, signature_type, api_key, api_secret, api_passphrase, funder) = match cache.get(&user_id) {
        Some(creds) => (
            creds.private_key.clone(),
            creds.signature_type,
            creds.api_key.clone(),
            creds.api_secret.clone(),
            creds.api_passphrase.clone(),
            creds.funder.clone(),
        ),
        None => {
            return Json(DepositWalletResponse {
                success: false,
                wallet_address: None,
                message: "No cached credentials found. Log in and set up API keys first.".to_string(),
            }).into_response();
        }
    };
    drop(cache);

    // Create a client with the credentials
    let client = match crate::trading::PolymarketClient::from_api_credentials(
        &private_key,
        signature_type,
        Some(crate::trading::polymarket::ApiKeyCreds {
            key: api_key,
            secret: api_secret,
            passphrase: api_passphrase,
        }),
        funder.as_deref(),
        None,
    ) {
        Ok(c) => c,
        Err(e) => {
            return Json(DepositWalletResponse {
                success: false,
                wallet_address: None,
                message: format!("Failed to create client: {e}"),
            }).into_response();
        }
    };

    // Query the relayer API
    match client.query_deposit_wallet_address_from_relayer().await {
        Ok(Some(addr)) => {
            // Store the address
            let pool = db.as_ref();
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO api_keys (user_id, key_name, key_value) VALUES (?, ?, ?)"
            )
            .bind(user_id)
            .bind("polymarket_deposit_wallet_address")
            .bind(&addr)
            .execute(pool)
            .await;

            // Update cache
            let mut cache = state.credential_cache.write().await;
            if let Some(creds) = cache.get_mut(&user_id) {
                creds.deposit_wallet_address = Some(addr.clone());
            }
            drop(cache);

            let deployed = crate::trading::polymarket::PolymarketClient::check_deposit_wallet_exists(&addr)
                .await
                .unwrap_or(false);

            Json(DepositWalletResponse {
                success: true,
                wallet_address: Some(addr),
                message: if deployed {
                    "Deposit wallet fetched from relayer and confirmed deployed on-chain.".to_string()
                } else {
                    "Deposit wallet address fetched from relayer.".to_string()
                },
            }).into_response()
        }
        Ok(None) => {
            Json(DepositWalletResponse {
                success: false,
                wallet_address: None,
                message: "No relayer API keys found. Create one on Polymarket.com Settings → API Keys first.".to_string(),
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch deposit wallet from relayer: {}", e);
            Json(DepositWalletResponse {
                success: false,
                wallet_address: None,
                message: format!("Failed to fetch from relayer: {e}"),
            }).into_response()
        }
    }
}

/// Get deposit wallet info for the authenticated user.
/// GET /funding/deposit-wallet-info
pub async fn get_deposit_wallet_info(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;

    // Check the credential cache first
    let cache = state.credential_cache.read().await;
    let cached_addr = cache.get(&user_id).and_then(|c| c.deposit_wallet_address.clone());
    drop(cache);

    if let Some(ref addr) = cached_addr {
        let exists = crate::trading::polymarket::PolymarketClient::check_deposit_wallet_exists(addr)
            .await
            .unwrap_or(false);
        return Json(DepositWalletInfoResponse {
            eoa_address: String::new(),
            deposit_wallet_address: Some(addr.clone()),
            deposit_wallet_deployed: Some(exists),
            message: if exists {
                "Deposit wallet found and deployed on-chain.".to_string()
            } else {
                "Deposit wallet address known but not yet deployed on-chain.".to_string()
            },
        }).into_response();
    }

    Json(DepositWalletInfoResponse {
        eoa_address: String::new(),
        deposit_wallet_address: None,
        deposit_wallet_deployed: None,
        message: "No deposit wallet saved. Get the address from Polymarket.com Settings → API Keys.".to_string(),
    }).into_response()
}

#[derive(Debug, Serialize)]
pub struct DepositWalletInfoResponse {
    pub eoa_address: String,
    pub deposit_wallet_address: Option<String>,
    pub deposit_wallet_deployed: Option<bool>,
    pub message: String,
}

// ─── Deposit Wallet Deploy ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DeployWalletResponse {
    pub success: bool,
    pub transaction_hash: Option<String>,
    pub wallet_address: Option<String>,
    pub error: Option<String>,
}

/// Deploy the deposit wallet contract via the Deposit Wallet Factory.
/// Requires MATIC in the EOA wallet for gas.
/// POST /funding/deploy-deposit-wallet
pub async fn deploy_deposit_wallet(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;

    // Get credentials from cache
    let cache = state.credential_cache.read().await;
    let (private_key, wallet_address, deposit_wallet_addr) = match cache.get(&user_id) {
        Some(creds) => (
            creds.private_key.clone(),
            creds.wallet_address.clone(),
            creds.deposit_wallet_address.clone(),
        ),
        None => {
            return Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: None,
                error: Some("No cached credentials found. Log in and set up API keys first.".to_string()),
            }).into_response();
        }
    };
    drop(cache);

    if wallet_address.is_empty() {
        return Json(DeployWalletResponse {
            success: false,
            transaction_hash: None,
            wallet_address: None,
            error: Some("No EOA wallet address found in credentials.".to_string()),
        }).into_response();
    }

    // Parse the EOA wallet address
    let user_addr: H160 = match wallet_address.parse() {
        Ok(a) => a,
        Err(e) => {
            return Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: None,
                error: Some(format!("Invalid wallet address: {e}")),
            }).into_response();
        }
    };

    // Create the wallet from private key
    let key_bytes = match hex::decode(private_key.trim_start_matches("0x")) {
        Ok(b) => b,
        Err(e) => {
            return Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: None,
                error: Some(format!("Invalid private key: {e}")),
            }).into_response();
        }
    };
    let wallet = match ethers::signers::LocalWallet::from_bytes(&key_bytes) {
        Ok(w) => w,
        Err(e) => {
            return Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: None,
                error: Some(format!("Invalid wallet: {e}")),
            }).into_response();
        }
    };

    // Check MATIC balance first
    match check_matic_balance(&wallet_address).await {
        Ok(bal) if bal < 0.1 => {
            return Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: deposit_wallet_addr,
                error: Some(format!(
                    "Insufficient MATIC balance ({:.4} MATIC). Need at least 0.1 MATIC for gas.", bal
                )),
            }).into_response();
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("Could not check MATIC balance: {}", e);
        }
    }

    // Compute the salt: user address left-padded to 32 bytes (CREATE2 pattern)
    let salt = {
        let mut s = [0u8; 32];
        let addr_bytes: [u8; 20] = user_addr.into();
        s[12..].copy_from_slice(&addr_bytes);
        s
    };

    // Encode deploy(address[],bytes32[])
    let factory_addr: H160 = match crate::trading::polymarket::DEPOSIT_WALLET_FACTORY.parse() {
        Ok(a) => a,
        Err(e) => {
            return Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: None,
                error: Some(format!("Invalid factory address: {e}")),
            }).into_response();
        }
    };
    let data = crate::trading::polymarket::encode_deploy_call(user_addr, salt);

    // Send the transaction
    match sign_and_send_tx(&wallet, factory_addr, data, U256::zero(), Some(300_000)).await {
        Ok(tx_hash) => {
            tracing::info!("Deposit wallet deploy tx sent: {}", tx_hash);
            
            // Wait for confirmation
            match wait_for_tx_receipt(&tx_hash).await {
                Ok(()) => {
                    tracing::info!("Deposit wallet deploy confirmed: tx={}", tx_hash);
                    
                    Json(DeployWalletResponse {
                        success: true,
                        transaction_hash: Some(tx_hash),
                        wallet_address: deposit_wallet_addr,
                        error: None,
                    }).into_response()
                }
                Err(e) => {
                    Json(DeployWalletResponse {
                        success: true,
                        transaction_hash: Some(tx_hash),
                        wallet_address: deposit_wallet_addr,
                        error: Some(format!("Transaction sent but waiting for confirmation failed: {e}")),
                    }).into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!("Deposit wallet deploy failed: {}", e);
            Json(DeployWalletResponse {
                success: false,
                transaction_hash: None,
                wallet_address: deposit_wallet_addr,
                error: Some(format!("Deploy transaction failed: {e}")),
            }).into_response()
        }
    }
}


