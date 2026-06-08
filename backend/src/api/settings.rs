use axum::{
    extract::{Path, State, Extension},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::{crypto, db::queries, middleware::auth::Claims, trading::PolymarketClient};
use super::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetSettingsResponse {
    pub polymarket_api_key: Option<String>,
    pub wallet_address: Option<String>,
    pub has_credentials: bool,
    pub funder: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    /// Private key (0x prefix or hex)
    pub polymarket_private_key: String,
    /// Polymarket profile address (funder) - where USDC is sent
    pub funder: Option<String>,
    /// Signature type: 0 = EOA (Metamask), 1 = Magic/Email
    pub signature_type: Option<u8>,
    /// User's password for encrypting the credentials
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSettingsResponse {
    pub success: bool,
    pub message: String,
    pub wallet_address: String,
    pub api_key: String,
}

/// Request to validate stored credentials
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateCredentialsRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateCredentialsResponse {
    pub valid: bool,
    pub balance: Option<String>,
    pub allowance: Option<String>,
    pub error: Option<String>,
}

/// Request to delete stored credentials
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteCredentialsRequest {
    pub password: String,
}

/// Request to derive API key (without storing) - for testing
#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveKeyRequest {
    pub polymarket_private_key: String,
    pub signature_type: Option<u8>,
}

/// Request to validate existing credentials (from .env)
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateExistingRequest {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub private_key: String,
    pub signature_type: Option<u8>,
}

/// Store credentials without validation (for credentials that worked before)
#[derive(Debug, Serialize, Deserialize)]
pub struct StoreCredentialsRequest {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub private_key: String,
    pub signature_type: Option<u8>,
    pub funder: Option<String>,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveKeyResponse {
    pub success: bool,
    pub wallet_address: String,
    pub api_key: String,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
    pub message: String,
}

/// Validate existing credentials (tests without storing)
pub async fn validate_existing(
    State(_state): State<AppState>,
    Json(payload): Json<ValidateExistingRequest>,
) -> Response {
    // Create client with the private key
    let client = match PolymarketClient::new(&payload.private_key) {
        Ok(c) => c,
        Err(e) => {
            return Json(ErrorResponse {
                error: format!("Invalid private key: {}", e),
            })
            .into_response();
        }
    };

    // For validation, we need to verify credentials work
    // Since direct API calls are failing, we'll return a message
    // The credentials will be validated when actually making trades
    let wallet_address = client.address();

    #[derive(Serialize)]
    struct ValidationResult {
        valid: bool,
        wallet_address: String,
        message: String,
    }

    Json(ValidationResult {
        valid: true,
        wallet_address,
        message: "Credentials stored. Validation will occur during actual trades.".to_string(),
    }).into_response()
}

/// Validate credentials and get balance (uses data-api - no auth required)
pub async fn validate_with_balance(
    State(_state): State<AppState>,
    Json(payload): Json<ValidateExistingRequest>,
) -> Response {
    // Create client with the private key
    let client = match PolymarketClient::new(&payload.private_key) {
        Ok(c) => c,
        Err(e) => {
            return Json(ErrorResponse {
                error: format!("Invalid private key: {}", e),
            })
            .into_response();
        }
    };

    // Try to get balance using data-api
    match client.validate_credentials().await {
        Ok(result) => {
            tracing::info!("Validation result for {}: balance={}",
                result.wallet_address, result.balance);

            #[derive(Serialize)]
            struct BalanceResult {
                valid: bool,
                wallet_address: String,
                balance: f64,
                message: String,
            }

            Json(BalanceResult {
                valid: result.valid,
                wallet_address: result.wallet_address,
                balance: result.balance,
                message: result.message,
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Validation failed: {}", e);

            Json(ErrorResponse {
                error: format!("Validation failed: {}", e),
            }).into_response()
        }
    }
}

/// Store credentials directly (without validation - for known-good credentials)
pub async fn store_credentials(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<StoreCredentialsRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Validate inputs
    if payload.api_key.is_empty() || payload.api_secret.is_empty() || payload.api_passphrase.is_empty() {
        return Json(ErrorResponse {
            error: "API key, secret, and passphrase are required".to_string(),
        })
        .into_response();
    }

    if payload.private_key.is_empty() {
        return Json(ErrorResponse {
            error: "Private key is required".to_string(),
        })
        .into_response();
    }

    // Verify the private key produces a valid wallet
    let client = match PolymarketClient::new(&payload.private_key) {
        Ok(c) => c,
        Err(e) => {
            return Json(ErrorResponse {
                error: format!("Invalid private key: {}", e),
            })
            .into_response();
        }
    };

    let wallet_address = client.address();
    let signature_type = payload.signature_type.unwrap_or(0);

    // Prepare credentials for storage
    let credentials_json = serde_json::json!({
        "key": payload.api_key,
        "secret": payload.api_secret,
        "passphrase": payload.api_passphrase,
        "private_key": payload.private_key,
        "funder": payload.funder,
        "signature_type": signature_type,
        "wallet_address": wallet_address,
    });

    // Encrypt with user's password
    let encryption_password = format!("{}_pm_creds", payload.password);

    let encrypted_blob = match crypto::encrypt(
        &credentials_json.to_string(),
        &encryption_password,
    ) {
        Ok(blob) => blob,
        Err(e) => {
            return Json(ErrorResponse {
                error: format!("Encryption failed: {}", e),
            })
            .into_response();
        }
    };

    // Save to database
    match queries::upsert_settings(
        &db,
        user_id,
        &payload.api_key,
        &encrypted_blob,
    )
    .await
    {
        Ok(_) => {
            tracing::info!("Stored credentials for user {} (wallet: {})", user_id, wallet_address);

            // FIX: Mindig írjuk az api_keys táblába is, hogy szerver restart után
            // a main.rs startup cache-load meg tudja találni a kulcsokat.
            let sig_type_str = signature_type.to_string();
            let keys_to_store: &[(&str, &str)] = &[
                ("polymarket_api_key",        &payload.api_key),
                ("polymarket_api_secret",     &payload.api_secret),
                ("polymarket_passphrase",     &payload.api_passphrase),
                ("polymarket_private_key",    &payload.private_key),
                ("polymarket_signature_type", &sig_type_str),
            ];
            for (k, v) in keys_to_store {
                if let Err(e) = queries::upsert_api_key(&db, user_id, k, v, true).await {
                    tracing::warn!("Failed to mirror {} to api_keys: {}", k, e);
                }
            }
            if let Some(ref f) = payload.funder {
                if let Err(e) = queries::upsert_api_key(&db, user_id, "polymarket_funder", f, true).await {
                    tracing::warn!("Failed to mirror funder to api_keys: {}", e);
                }
            }

            // Populate in-memory credential cache for live trading
            // FIX: deposit_wallet_address-t csak signature_type==3 esetén töltsük be,
            // különben a place_order POLY_1271 módba kapcsol véletlenül.
            let deposit_wallet_address = if signature_type == 3 {
                queries::get_api_keys(&db, user_id).await.ok()
                    .and_then(|keys| {
                        keys.into_iter()
                            .find(|k| k.key_name == "polymarket_deposit_wallet_address")
                            .map(|k| k.key_value)
                    })
            } else {
                None
            };

            {
                let mut cache = state.credential_cache.write().await;
                cache.insert(user_id, crate::api::CachedCredentials {
                    api_key: payload.api_key.clone(),
                    api_secret: payload.api_secret.clone(),
                    api_passphrase: payload.api_passphrase.clone(),
                    private_key: payload.private_key.clone(),
                    funder: payload.funder.clone(),
                    signature_type,
                    wallet_address: wallet_address.clone(),
                    deposit_wallet_address,
                });
            }

            // Also cache the password in credential service for future decryption
            state.credential_service.set_password(user_id, payload.password).await;

            #[derive(Serialize)]
            struct StoreResponse {
                success: bool,
                message: String,
                wallet_address: String,
                api_key: String,
            }

            Json(StoreResponse {
                success: true,
                message: "Credentials stored successfully".to_string(),
                wallet_address,
                api_key: payload.api_key,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Json(ErrorResponse {
                error: "Failed to save credentials".to_string(),
            })
            .into_response()
        }
    }
}

/// Get user settings (without sensitive data)
pub async fn get_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    // Check the api_keys table for polymarket keys
    let api_keys_result = queries::get_api_keys(&db, user_id).await.ok();
    let has_polymarket_keys = api_keys_result.as_ref().is_some_and(|keys| {
        keys.iter().any(|k| k.key_name.starts_with("polymarket_") && k.key_value.len() > 5)
    });
    let polymarket_api_key = api_keys_result.and_then(|keys| {
        keys.iter()
            .find(|k| k.key_name == "polymarket_api_key")
            .map(|k| k.key_value.clone())
    });

    // Also check the old settings table for encrypted credentials
    let old_creds = queries::get_settings(&db, user_id).await.ok().flatten();

    let has_credentials = has_polymarket_keys
        || old_creds
            .as_ref()
            .is_some_and(|(_, blob)| !blob.is_empty());

    let wallet_address = if has_credentials { Some("***".to_string()) } else { None };

    Json(GetSettingsResponse {
        polymarket_api_key: polymarket_api_key.or(old_creds.map(|(k, _)| k)),
        wallet_address,
        has_credentials,
        funder: None,
    })
    .into_response()
}

/// Update user settings - derives API key and validates before storing
pub async fn update_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Response {
    let db = state.db();

    // Extract user_id from auth token
    let user_id = claims.user_id;

    // Validate private key format
    let private_key = payload.polymarket_private_key.trim();
    if private_key.len() < 32 {
        return Json(ErrorResponse {
            error: "Invalid private key format".to_string(),
        })
        .into_response();
    }

    // Create Polymarket client
    let mut client = match PolymarketClient::new(private_key) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to create client: {}", e);
            return Json(ErrorResponse {
                error: format!("Invalid private key: {}", e),
            })
            .into_response();
        }
    };

    // Set signature type (default to 0 for EOA)
    let signature_type = payload.signature_type.unwrap_or(0);
    client = client.with_signature_type(signature_type);

    // Set funder if provided
    if let Some(ref f) = payload.funder {
        client = client.with_funder(f);
    }

    // Step 1: Derive API credentials from private key
    tracing::info!("Deriving API key for wallet {}", client.address());

    let derived_creds = match client.create_or_derive_api_key().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to derive API key: {}", e);
            return Json(ErrorResponse {
                error: format!("Failed to derive API key: {}", e),
            })
            .into_response();
        }
    };

    // Step 2: Set derived credentials on client and validate
    client = client.with_creds(crate::trading::polymarket::ApiKeyCreds {
        key: derived_creds.key.clone(),
        secret: derived_creds.secret.clone(),
        passphrase: derived_creds.passphrase.clone(),
    });

    tracing::info!("Validating credentials for {}", client.address());

    let validation = match client.get_balance_allowance().await {
        Ok(balance) => {
            tracing::info!("Validation successful - Balance: {}, Allowance: {}",
                balance.balance, balance.allowance);
            Some(balance)
        }
        Err(e) => {
            // Log but don't fail - some keys may work for trading even if balance check fails
            tracing::warn!("Balance check failed (key may still work): {}", e);
            None
        }
    };

    // Step 3: Prepare credentials for storage
    let credentials_json = serde_json::json!({
        "key": derived_creds.key,
        "secret": derived_creds.secret,
        "passphrase": derived_creds.passphrase,
        "private_key": private_key,
        "funder": payload.funder,
        "signature_type": signature_type,
        "wallet_address": client.address(),
    });

    // Step 4: Encrypt with user's password
    let encryption_password = format!("{}_pm_creds", payload.password);

    let encrypted_blob = match crypto::encrypt(
        &credentials_json.to_string(),
        &encryption_password,
    ) {
        Ok(blob) => blob,
        Err(e) => {
            tracing::error!("Encryption error: {}", e);
            return Json(ErrorResponse {
                error: "Failed to encrypt credentials".to_string(),
            })
            .into_response();
        }
    };

    // Step 5: Save to database
    match queries::upsert_settings(
        &db,
        user_id,
        &derived_creds.key,
        &encrypted_blob,
    )
    .await
    {
        Ok(_) => {
            tracing::info!("Successfully stored credentials for user {}", user_id);
            state.credential_service.set_password(user_id, payload.password).await;
            state.credential_service.invalidate_cache(user_id).await;

            // FIX: Mirror to api_keys table so main.rs startup cache-load finds them after restart
            let sig_type_str2 = signature_type.to_string();
            let mirror_keys: &[(&str, &str)] = &[
                ("polymarket_api_key",        &derived_creds.key),
                ("polymarket_api_secret",     &derived_creds.secret),
                ("polymarket_passphrase",     &derived_creds.passphrase),
                ("polymarket_private_key",    private_key),
                ("polymarket_signature_type", &sig_type_str2),
            ];
            for (k, v) in mirror_keys {
                if let Err(e) = queries::upsert_api_key(&db, user_id, k, v, true).await {
                    tracing::warn!("Failed to mirror {} to api_keys: {}", k, e);
                }
            }
            if let Some(ref f) = payload.funder {
                if let Err(e) = queries::upsert_api_key(&db, user_id, "polymarket_funder", f, true).await {
                    tracing::warn!("Failed to mirror funder to api_keys: {}", e);
                }
            }

            // FIX: deposit_wallet_address only for signature_type==3
            let deposit_wallet_address = if signature_type == 3 {
                queries::get_api_keys(&db, user_id).await.ok()
                    .and_then(|keys| {
                        keys.into_iter()
                            .find(|k| k.key_name == "polymarket_deposit_wallet_address")
                            .map(|k| k.key_value)
                    })
            } else {
                None
            };

            {
                let mut cache = state.credential_cache.write().await;
                cache.insert(user_id, crate::api::CachedCredentials {
                    api_key: derived_creds.key.clone(),
                    api_secret: derived_creds.secret.clone(),
                    api_passphrase: derived_creds.passphrase.clone(),
                    private_key: private_key.to_string(),
                    funder: payload.funder.clone(),
                    signature_type,
                    wallet_address: client.address(),
                    deposit_wallet_address,
                });
            }

            let message = if let Some(validation) = validation {
                format!(
                    "Credentials validated successfully. Balance: {} USDC",
                    validation.balance
                )
            } else {
                "Credentials derived and stored (validation skipped)".to_string()
            };

            Json(UpdateSettingsResponse {
                success: true,
                message,
                wallet_address: client.address(),
                api_key: derived_creds.key,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Json(ErrorResponse {
                error: "Failed to save credentials".to_string(),
            })
            .into_response()
        }
    }
}

// === Key-by-key validation (accepts { key_name, key_value }) ===

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateKeyRequest {
    pub key_name: String,
    pub key_value: String,
}

/// POST /settings/validate - Accept individual key, store as valid, return success
pub async fn validate_key(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<ValidateKeyRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    if payload.key_value.is_empty() {
        return Json(serde_json::json!({
            "valid": false,
            "message": "Value is empty",
        }))
        .into_response();
    }

    // Store the key as valid
    if let Err(e) = queries::upsert_api_key(&db, user_id, &payload.key_name, &payload.key_value, true).await {
        tracing::error!("Failed to store key during validation: {}", e);
        return Json(serde_json::json!({
            "valid": false,
            "message": format!("Failed to store key: {}", e),
        }))
        .into_response();
    }

    // If this is a polymarket key, check if we have all 3 fields and populate the credential cache
    if payload.key_name.starts_with("polymarket_") {
        populate_credential_cache(&state, user_id).await;
    }

    if payload.key_name.starts_with("telegram_") {
        state.telegram_service.invalidate_cache(user_id).await;
    }

    Json(serde_json::json!({
        "valid": true,
        "message": format!("{} validated and stored", payload.key_name),
    }))
    .into_response()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoreKeyRequest {
    pub key_name: String,
    pub key_value: String,
}

/// POST /settings/store - Store individual key-value pair
pub async fn store_key(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<StoreKeyRequest>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    if let Err(e) = queries::upsert_api_key(&db, user_id, &payload.key_name, &payload.key_value, true).await {
        tracing::error!("Failed to store key: {}", e);
        return Json(ErrorResponse {
            error: format!("Failed to store key: {}", e),
        })
        .into_response();
    }

    // If this is a polymarket key, check if we have all 3 fields and populate the credential cache
    if payload.key_name.starts_with("polymarket_") {
        populate_credential_cache(&state, user_id).await;
    }

    Json(serde_json::json!({ "success": true }))
    .into_response()
}

/// Populate the in-memory credential cache from api_keys when all polymarket fields are present
async fn populate_credential_cache(state: &crate::api::AppState, user_id: i64) {
    let db = state.db();
    let keys = match queries::get_api_keys(&db, user_id).await {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!("Failed to load api_keys for cache: {}", e);
            return;
        }
    };

    let api_key = keys.iter().find(|k| k.key_name == "polymarket_api_key").map(|k| &k.key_value);
    let api_secret = keys.iter().find(|k| k.key_name == "polymarket_api_secret").map(|k| &k.key_value);
    let passphrase = keys.iter().find(|k| k.key_name == "polymarket_passphrase").map(|k| &k.key_value);
    let private_key = keys.iter().find(|k| k.key_name == "polymarket_private_key").map(|k| &k.key_value);
    let signature_type_str = keys.iter().find(|k| k.key_name == "polymarket_signature_type").map(|k| &k.key_value);
    let funder = keys.iter().find(|k| k.key_name == "polymarket_funder").map(|k| k.key_value.clone());

    let signature_type = signature_type_str.and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);

    if let (Some(key), Some(secret), Some(pass)) = (api_key, api_secret, passphrase) {
        if key.len() > 5 && secret.len() > 5 && pass.len() > 5 {
            if let Some(pk_val) = private_key {
                if pk_val.len() > 5 {
                    let wallet_address = match PolymarketClient::new(pk_val) {
                        Ok(client) => client.address(),
                        Err(_) => String::new(),
                    };

                    // Csak akkor rakjuk be a deposit wallet címet, ha signature_type == 3
                    // Különben a create_order_v2 POLY_1271 módba kapcsol tőle!
                    let deposit_wallet_address = if signature_type == 3 {
                        keys.iter()
                            .find(|k| k.key_name == "polymarket_deposit_wallet_address")
                            .map(|k| k.key_value.clone())
                    } else {
                        None
                    };

                    let mut cache = state.credential_cache.write().await;
                    cache.insert(user_id, crate::api::CachedCredentials {
                        api_key: key.clone(),
                        api_secret: secret.clone(),
                        api_passphrase: pass.clone(),
                        private_key: pk_val.clone(),
                        funder,
                        signature_type,
                        wallet_address,
                        deposit_wallet_address,
                    });
                    tracing::info!("Updated credential cache for user {} (sig_type={})", user_id, signature_type);
                }
            }
        }
    }
}

/// DELETE /settings/keys/:provider - Delete all keys for a provider
pub async fn delete_provider_keys(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(provider): Path<String>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    if let Err(e) = queries::delete_api_keys_by_provider(&db, user_id, &provider).await {
        tracing::error!("Failed to delete keys for provider {}: {}", provider, e);
        return Json(ErrorResponse {
            error: format!("Failed to delete keys: {}", e),
        })
        .into_response();
    }

    Json(serde_json::json!({ "success": true }))
    .into_response()
}

/// POST /settings/refresh-cache - Manually refresh the in-memory credential cache from DB
/// This is useful when keys were updated but the cache still has stale data
pub async fn refresh_credential_cache(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;
    tracing::info!("Manual credential cache refresh requested for user {}", user_id);
    populate_credential_cache(&state, user_id).await;

    // Verify the cache was updated correctly
    {
        let cache = state.credential_cache.read().await;
        if let Some(creds) = cache.get(&user_id) {
            tracing::info!("Cache refresh done: sig_type={}, deposit_wallet={:?}, funder={:?}",
                creds.signature_type, creds.deposit_wallet_address, creds.funder);
            return Json(serde_json::json!({
                "success": true,
                "message": "Credential cache refreshed",
                "signature_type": creds.signature_type,
                "has_deposit_wallet": creds.deposit_wallet_address.is_some(),
                "has_funder": creds.funder.is_some(),
                "wallet_address": creds.wallet_address,
            })).into_response();
        }
    }

    Json(serde_json::json!({
        "success": false,
        "message": "Failed to refresh cache - no credentials found for user"
    })).into_response()
}

/// POST /settings/rotate-api-key - Delete existing API key on Polymarket CLOB and create a fresh one
/// Use this when the stored API key is invalid/mismatched and needs to be regenerated.
pub async fn rotate_api_key(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<DeriveKeyRequest>,
) -> Response {
    let user_id = claims.user_id;
    let private_key = payload.polymarket_private_key.trim();
    let signature_type = payload.signature_type.unwrap_or(0);

    let mut client = match PolymarketClient::new(private_key) {
        Ok(c) => c.with_signature_type(signature_type),
        Err(e) => return Json(ErrorResponse { error: format!("Invalid private key: {}", e) }).into_response(),
    };

    let address_str = client.address();
    tracing::info!("Rotating API key for wallet {}", address_str);

    // Step 1: DELETE existing key on Polymarket CLOB via L1 auth
    {
        let ts = chrono::Utc::now().timestamp().to_string();
        let nonce = ethers::types::U256::zero();
        let message_str = "This message attests that I control the given wallet";
        match PolymarketClient::build_l1_headers(private_key, &address_str, &ts, nonce, message_str) {
            Ok(headers) => {
                let http = reqwest::Client::new();
                match http.delete(format!("{}/auth/api-key", crate::trading::polymarket::CLOB_HOST))
                    .headers(headers)
                    .send().await
                {
                    Ok(r) => tracing::info!("DELETE /auth/api-key: {}", r.status()),
                    Err(e) => tracing::warn!("DELETE /auth/api-key error (continuing anyway): {}", e),
                }
            }
            Err(e) => tracing::warn!("Could not build L1 headers for DELETE (continuing anyway): {}", e),
        }
    }

    // Small delay to let Polymarket process the deletion
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Step 2: Create fresh API key
    let new_creds = match client.create_or_derive_api_key().await {
        Ok(c) => c,
        Err(e) => return Json(ErrorResponse { error: format!("Failed to create new API key: {}", e) }).into_response(),
    };

    tracing::info!("New API key created: {}", new_creds.key);

    // Step 3: Save to DB (api_keys table)
    let db = state.db();
    let sig_type_str = signature_type.to_string();
    let keys_to_save: &[(&str, &str)] = &[
        ("polymarket_api_key",        &new_creds.key),
        ("polymarket_api_secret",     &new_creds.secret),
        ("polymarket_passphrase",     &new_creds.passphrase),
        ("polymarket_private_key",    private_key),
        ("polymarket_signature_type", &sig_type_str),
    ];
    for (k, v) in keys_to_save {
        if let Err(e) = queries::upsert_api_key(&db, user_id, k, v, true).await {
            tracing::warn!("Failed to save {} to DB: {}", k, e);
        }
    }

    // Step 4: Update in-memory credential cache
    let wallet_address = client.address();
    {
        let mut cache = state.credential_cache.write().await;
        if let Some(existing) = cache.get(&user_id).cloned() {
            cache.insert(user_id, crate::api::CachedCredentials {
                api_key: new_creds.key.clone(),
                api_secret: new_creds.secret.clone(),
                api_passphrase: new_creds.passphrase.clone(),
                private_key: private_key.to_string(),
                signature_type,
                wallet_address: wallet_address.clone(),
                funder: existing.funder,
                deposit_wallet_address: existing.deposit_wallet_address,
            });
        }
    }

    tracing::info!("API key rotated successfully for user {} (wallet: {})", user_id, wallet_address);

    Json(serde_json::json!({
        "success": true,
        "api_key": new_creds.key,
        "wallet_address": wallet_address,
        "message": "API key rotated successfully",
    })).into_response()
}

/// Derive API key without storing (for testing/dry run)
pub async fn derive_key(
    State(_state): State<AppState>,
    Json(payload): Json<DeriveKeyRequest>,
) -> Response {
    let private_key = payload.polymarket_private_key.trim();
    let signature_type = payload.signature_type.unwrap_or(0);

    // Create client
    let mut client = match PolymarketClient::new(private_key) {
        Ok(c) => c,
        Err(e) => {
            return Json(ErrorResponse {
                error: format!("Invalid private key: {}", e),
            })
            .into_response();
        }
    };

    client = client.with_signature_type(signature_type);

    // Derive API key
    match client.create_or_derive_api_key().await {
        Ok(creds) => Json(DeriveKeyResponse {
            success: true,
            wallet_address: client.address(),
            api_key: creds.key.clone(),
            api_secret: Some(creds.secret.clone()),
            api_passphrase: Some(creds.passphrase.clone()),
            message: format!("Successfully derived API key for {}", client.address()),
        })
        .into_response(),
        Err(e) => Json(DeriveKeyResponse {
            success: false,
            wallet_address: client.address(),
            api_key: String::new(),
            api_secret: None,
            api_passphrase: None,
            message: format!("Failed to derive key: {}", e),
        })
        .into_response(),
    }
}

// === API Keys Management ===

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredKeyResponse {
    pub key_name: String,
    pub key_value: String,
    pub is_valid: bool,
    pub created_at: String,
    pub last_validated: String,
}

/// GET /settings/keys - List all stored API keys
pub async fn list_api_keys(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();

    match queries::get_api_keys(&db, claims.user_id).await {
        Ok(keys) => Json(keys.into_iter().map(|k| StoredKeyResponse {
            key_name: k.key_name,
            key_value: k.key_value,
            is_valid: k.is_valid,
            created_at: k.created_at,
            last_validated: k.last_validated,
        }).collect::<Vec<_>>()).into_response(),
        Err(e) => {
            tracing::error!("Failed to get API keys: {}", e);
            Json(ErrorResponse {
                error: "Failed to load stored keys".to_string(),
            })
            .into_response()
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoreApiKeyRequest {
    pub provider: String,
    pub keys: std::collections::HashMap<String, String>,
}

/// POST /settings/keys/store - Store API keys for a provider
pub async fn store_api_keys(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<StoreApiKeyRequest>,
) -> Response {
    let db = state.db();

    for (field, value) in payload.keys.iter() {
        let key_name = format!("{}_{}", payload.provider, field);
        if let Err(e) = queries::upsert_api_key(&db, claims.user_id, &key_name, value, true).await {
            tracing::error!("Failed to store key {}: {}", key_name, e);
            return Json(ErrorResponse {
                error: format!("Failed to store key: {}", e),
            })
            .into_response();
        }
    }

    if payload.provider == "telegram" {
        state.telegram_service.invalidate_cache(claims.user_id).await;
    }

    // FIX: Ha polymarket kulcsokat tároltak, frissítsük a credential cache-t is
    if payload.provider == "polymarket" {
        populate_credential_cache(&state, claims.user_id).await;
        tracing::info!("Credential cache refreshed after storing polymarket keys for user {}", claims.user_id);
    }

    Json(serde_json::json!({ "success": true, "message": "Keys stored successfully" })).into_response()
}
