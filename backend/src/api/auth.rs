use axum::{
    extract::{Extension, State},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::db::queries;
use crate::db;
use crate::middleware::auth::Claims;
use super::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: i64,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Response {
    let db = state.db();

    if payload.username.len() < 3 {
        return Json(ErrorResponse {
            error: "Username must be at least 3 characters".to_string(),
        }).into_response();
    }

    if payload.password.len() < 6 {
        return Json(ErrorResponse {
            error: "Password must be at least 6 characters".to_string(),
        }).into_response();
    }

    match queries::find_user_by_username(&db, &payload.username).await {
        Ok(Some(_)) => {
            return Json(ErrorResponse {
                error: "Username already exists".to_string(),
            }).into_response();
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Json(ErrorResponse {
                error: "Internal server error".to_string(),
            }).into_response();
        }
        _ => {}
    }

    let password_hash = match bcrypt::hash(&payload.password, 12) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Password hashing error: {}", e);
            return Json(ErrorResponse {
                error: "Internal server error".to_string(),
            }).into_response();
        }
    };

    match queries::create_user(&db, &payload.username, &password_hash).await {
        Ok(user_id) => {
            let user_count: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM users")
                .fetch_one(db.as_ref())
                .await;
            if user_count.is_ok_and(|c| c == 1) {
                let _ = db::seed_default_bots(db.as_ref(), user_id).await;
            }

            match generate_token(user_id, &payload.username) {
                Ok(token) => Json(AuthResponse {
                    token,
                    user_id,
                    username: payload.username,
                }).into_response(),
                Err(e) => {
                    tracing::error!("Token generation error: {}", e);
                    Json(ErrorResponse {
                        error: "Internal server error".to_string(),
                    }).into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!("User creation error: {}", e);
            Json(ErrorResponse {
                error: "Internal server error".to_string(),
            }).into_response()
        }
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Response {
    let db = state.db();

    let user = match queries::find_user_by_username(&db, &payload.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Json(ErrorResponse {
                error: "Invalid username or password".to_string(),
            }).into_response();
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Json(ErrorResponse {
                error: "Internal server error".to_string(),
            }).into_response();
        }
    };

    match bcrypt::verify(&payload.password, &user.2) {
        Ok(true) => {}
        Ok(false) => {
            return Json(ErrorResponse {
                error: "Invalid username or password".to_string(),
            }).into_response();
        }
        Err(e) => {
            tracing::error!("Password verification error: {}", e);
            return Json(ErrorResponse {
                error: "Internal server error".to_string(),
            }).into_response();
        }
    };

    state.credential_service.set_password(user.0, payload.password.clone()).await;

    if let Ok(creds) = state.credential_service.get_credentials(&db, user.0).await {
        // Load signature_type and deposit_wallet_address from DB api_keys table.
        // The credential service may return stale values (e.g., signature_type=0)
        // because it reads from encrypted JSON saved during initial setup.
        let (signature_type, deposit_wallet_address) = if let Ok(keys) = crate::db::queries::get_api_keys(&db, user.0).await {
            let sig = keys.iter()
                .find(|k| k.key_name == "polymarket_signature_type")
                .and_then(|k| k.key_value.parse::<u8>().ok())
                .unwrap_or(creds.signature_type);
            let deposit = keys.iter()
                .find(|k| k.key_name == "polymarket_deposit_wallet_address")
                .map(|k| k.key_value.clone());
            (sig, deposit)
        } else {
            (creds.signature_type, None)
        };

        tracing::info!("Populated credential_cache for user {} on login (sig_type={}, deposit_wallet={:?})", user.0, signature_type, &deposit_wallet_address);

        let mut cache = state.credential_cache.write().await;
        cache.insert(user.0, crate::api::CachedCredentials {
            api_key: creds.api_key,
            api_secret: creds.api_secret,
            api_passphrase: creds.api_passphrase,
            private_key: creds.private_key,
            funder: creds.funder,
            signature_type,
            wallet_address: creds.wallet_address,
            deposit_wallet_address,
        });
    }

    match generate_token(user.0, &user.1) {
        Ok(token) => Json(AuthResponse {
            token,
            user_id: user.0,
            username: user.1,
        }).into_response(),
        Err(e) => {
            tracing::error!("Token generation error: {}", e);
            Json(ErrorResponse {
                error: "Internal server error".to_string(),
            }).into_response()
        }
    }
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    let db = state.db();
    let user_id = claims.user_id;

    match queries::find_user_by_id(&db, user_id).await {
        Ok(Some((id, username))) => Json(UserResponse { id, username }).into_response(),
        Ok(None) => Json(ErrorResponse { error: "User not found".to_string() }).into_response(),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Json(ErrorResponse { error: "Internal server error".to_string() }).into_response()
        }
    }
}

fn generate_token(user_id: i64, username: &str) -> Result<String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Claims<'a> {
        user_id: i64,
        username: &'a str,
        exp: i64,
    }

    let secret = std::env::var("JWT_SECRET")
    .unwrap_or_else(|_| "CHANGE_ME_SET_JWT_SECRET_ENV_VAR".to_string());

    // Token érvényes 1 évig - nem jár le újraindítás után
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(365))
        .ok_or_else(|| {
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken)
        })?
        .timestamp();

    let claims = Claims {
        user_id,
        username,
        exp,
    };

    encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}
