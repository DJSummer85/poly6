use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;

use super::AppState;
use crate::db;
use crate::middleware::auth::Claims;
use crate::trading::PolymarketClient;
use crate::services::credential_service::CredentialError;

#[derive(Serialize)]
pub struct UserBalanceResponse {
    pub balance: f64,
    pub wallet_address: String,
    pub has_credentials: bool,
    pub error: Option<String>,
}

/// GET /user/balance — Fetch real USDC balance from Polymarket API
/// Uses CredentialService (encrypted settings table) as primary source,
/// falling back to credential_cache for backwards compatibility.
pub async fn get_user_balance(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let user_id = claims.user_id;
    let db = state.db();

    // Try to get credentials from CredentialService (encrypted settings table)
    let creds = match state.credential_service.get_credentials(&db, user_id).await {
        Ok(c) => c,
        Err(CredentialError::PasswordNotCached) => {
            // Password not cached (server restart) — try legacy api_keys
            if let Some(legacy_creds) = load_legacy_api_keys(&state, user_id).await {
                return fetch_balance_with_creds(legacy_creds).await.into_response();
            }
            return Json(UserBalanceResponse {
                balance: 0.0,
                wallet_address: String::new(),
                has_credentials: false,
                error: Some("Password not cached — please re-login or re-save credentials in Settings.".to_string()),
            })
            .into_response();
        }
        Err(CredentialError::NotFound) => {
            // No encrypted credentials — check if legacy api_keys exist
            if let Some(legacy_creds) = load_legacy_api_keys(&state, user_id).await {
                return fetch_balance_with_creds(legacy_creds).await.into_response();
            }
            return Json(UserBalanceResponse {
                balance: 0.0,
                wallet_address: String::new(),
                has_credentials: false,
                error: Some("No Polymarket credentials configured. Add your API keys in Settings.".to_string()),
            })
            .into_response();
        }
        Err(CredentialError::PrivateKeyRequired) => {
            return Json(UserBalanceResponse {
                balance: 0.0,
                wallet_address: String::new(),
                has_credentials: false,
                error: Some("Private key required for wallet access. Re-save your credentials in Settings.".to_string()),
            })
            .into_response();
        }
        Err(e) => {
            // Decrypt error or other issue — log and fall back to legacy
            tracing::warn!("CredentialService error for user {}: {}", user_id, e);
            if let Some(legacy_creds) = load_legacy_api_keys(&state, user_id).await {
                return fetch_balance_with_creds(legacy_creds).await.into_response();
            }
            return Json(UserBalanceResponse {
                balance: 0.0,
                wallet_address: String::new(),
                has_credentials: false,
                error: Some(format!("Failed to load credentials: {}", e)),
            })
            .into_response();
        }
    };

    // Use the decrypted credentials to fetch balance
    fetch_balance_from_polymarket(&creds).await.into_response()
}

/// Load legacy plain-text API keys from api_keys table (backwards compatibility)
async fn load_legacy_api_keys(
    state: &AppState,
    user_id: i64,
) -> Option<crate::api::CachedCredentials> {
    let cache = state.credential_cache.read().await;
    if let Some(cached) = cache.get(&user_id) {
        if !cached.api_key.is_empty() {
            return Some(cached.clone());
        }
    }
    drop(cache);

    // Try loading from api_keys table
    let keys = db::queries::get_api_keys(&state.db, user_id).await.ok()?;

    let api_key = keys.iter().find(|k| k.key_name == "polymarket_api_key").map(|k| &k.key_value);
    let api_secret = keys.iter().find(|k| k.key_name == "polymarket_api_secret").map(|k| &k.key_value);
    let passphrase = keys.iter().find(|k| k.key_name == "polymarket_passphrase").map(|k| &k.key_value);

    let (key, secret, pass) = (api_key?, api_secret?, passphrase?);
    if key.is_empty() || secret.is_empty() || pass.is_empty() {
        return None;
    }

    let private_key = keys.iter().find(|k| k.key_name == "polymarket_private_key").map(|k| &k.key_value);
    let (pk, wallet) = if let Some(pk_val) = private_key {
        match PolymarketClient::new(pk_val) {
            Ok(client) => (pk_val.clone(), client.address()),
            Err(_) => (String::new(), String::new()),
        }
    } else {
        (String::new(), String::new())
    };

    let cached = crate::api::CachedCredentials {
        api_key: key.clone(),
        api_secret: secret.clone(),
        api_passphrase: pass.clone(),
        private_key: pk,
        funder: None,
        signature_type: 0,
        wallet_address: wallet,
    };

    // Populate cache for next time
    state.credential_cache.write().await.insert(user_id, cached.clone());
    Some(cached)
}

/// Fetch balance using credentials from CredentialService
async fn fetch_balance_from_polymarket(creds: &crate::services::credential_service::PolymarketCredentials) -> Response {
    if creds.private_key.is_empty() {
        return Json(UserBalanceResponse {
            balance: 0.0,
            wallet_address: creds.wallet_address.clone(),
            has_credentials: true,
            error: Some("Private key not configured. Add your Polymarket private key in Settings.".to_string()),
        })
        .into_response();
    }

    // Build PolymarketClient with credentials for HMAC auth
    let client = match PolymarketClient::new(&creds.private_key) {
        Ok(c) => c.with_creds(crate::trading::polymarket::ApiKeyCreds {
            key: creds.api_key.clone(),
            secret: creds.api_secret.clone(),
            passphrase: creds.api_passphrase.clone(),
        })
        .with_signature_type(creds.signature_type),
        Err(e) => {
            return Json(UserBalanceResponse {
                balance: 0.0,
                wallet_address: creds.wallet_address.clone(),
                has_credentials: true,
                error: Some(format!("Invalid private key: {}", e)),
            })
            .into_response();
        }
    };

    // Set funder if provided
    let client = if let Some(ref funder) = creds.funder {
        client.with_funder(funder)
    } else {
        client
    };

    match client.get_balance().await {
        Ok(usdc_balance) => {
            tracing::info!("Fetched balance for {}: {} USDC", creds.wallet_address, usdc_balance);
            Json(UserBalanceResponse {
                balance: usdc_balance,
                wallet_address: creds.wallet_address.clone(),
                has_credentials: true,
                error: None,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Balance fetch failed for {}: {}", creds.wallet_address, e);
            Json(UserBalanceResponse {
                balance: 0.0,
                wallet_address: creds.wallet_address.clone(),
                has_credentials: true,
                error: Some(format!("Failed to fetch balance: {}", e)),
            })
            .into_response()
        }
    }
}

/// Fetch balance using legacy CachedCredentials (backwards compat)
async fn fetch_balance_with_creds(creds: crate::api::CachedCredentials) -> Response {
    if creds.private_key.is_empty() {
        return Json(UserBalanceResponse {
            balance: 0.0,
            wallet_address: creds.wallet_address.clone(),
            has_credentials: true,
            error: Some("Private key not configured. Add your Polymarket private key in Settings.".to_string()),
        })
        .into_response();
    }

    let client = PolymarketClient::new(&creds.private_key)
        .map(|c| c.with_creds(crate::trading::polymarket::ApiKeyCreds {
            key: creds.api_key,
            secret: creds.api_secret,
            passphrase: creds.api_passphrase,
        }))
        .unwrap_or_else(|_| {
            // If private key is invalid, create a dummy client with zero balance
            panic!("Invalid private key in legacy credentials")
        });

    match client.get_balance().await {
        Ok(usdc_balance) => Json(UserBalanceResponse {
            balance: usdc_balance,
            wallet_address: creds.wallet_address,
            has_credentials: true,
            error: None,
        })
        .into_response(),
        Err(e) => Json(UserBalanceResponse {
            balance: 0.0,
            wallet_address: creds.wallet_address,
            has_credentials: true,
            error: Some(format!("Failed to fetch balance: {}", e)),
        })
        .into_response(),
    }
}
