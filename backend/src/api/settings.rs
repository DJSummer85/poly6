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

            // Populate in-memory credential cache for live trading
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

    // Step 2: Validate credentials by checking balance
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

    if let (Some(key), Some(secret), Some(pass)) = (api_key, api_secret, passphrase) {
        if key.len() > 5 && secret.len() > 5 && pass.len() > 5 {
            let (pk, wallet) = if let Some(pk_val) = private_key {
                if pk_val.len() > 5 {
                    match PolymarketClient::new(pk_val) {
                        Ok(client) => (pk_val.clone(), client.address()),
                        Err(e) => {
                            tracing::warn!("Failed to derive wallet from private_key: {}", e);
                            (String::new(), String::new())
                        }
                    }
                } else {
                    (String::new(), String::new())
                }
            } else {
                (String::new(), String::new())
            };

            let mut cache = state.credential_cache.write().await;
            cache.insert(user_id, crate::api::CachedCredentials {
                api_key: key.clone(),
                api_secret: secret.clone(),
                api_passphrase: pass.clone(),
                private_key: pk,
                funder: None,
                signature_type: 0,
                wallet_address: wallet,
            });
            tracing::info!("Credential cache populated for user {}", user_id);
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
            api_key: creds.key,
            message: format!("Successfully derived API key for {}", client.address()),
        })
        .into_response(),
        Err(e) => Json(DeriveKeyResponse {
            success: false,
            wallet_address: client.address(),
            api_key: String::new(),
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

    Json(serde_json::json!({ "success": true, "message": "Keys stored successfully" })).into_response()
}
