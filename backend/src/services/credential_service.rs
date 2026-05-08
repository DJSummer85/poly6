//! Credential Service
//!
//! Központi szolgáltatás a Polymarket API credentialsek kezeléséhez.
//! Felelős a credentialsek decrypteléséért, validálásáért és cache-eléséért.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{crypto, db::{queries, Db}, trading::PolymarketClient};

/// Teljes, decryptelt credential
#[derive(Clone, Debug)]
pub struct PolymarketCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub private_key: String,
    pub funder: Option<String>,
    pub signature_type: u8,
    pub wallet_address: String,
}

/// Credential service error types
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("No credentials found for user")]
    NotFound,

    #[error("Password not cached — user needs to re-login or re-save settings")]
    PasswordNotCached,

    #[error("Failed to decrypt credentials: {0}")]
    DecryptError(String),

    #[error("Failed to parse credentials: {0}")]
    ParseError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("Private key required for this operation")]
    PrivateKeyRequired,
}

/// Credential service - kezeli a felhasználói credentialseket
#[derive(Clone)]
pub struct CredentialService {
    /// In-memory cache decryptelt credentialsekhez (user_id -> credentials)
    cache: Arc<RwLock<HashMap<i64, PolymarketCredentials>>>,
    /// Password cache - a felhasználó által megadott jelszó (user_id -> password)
    password_cache: Arc<RwLock<HashMap<i64, String>>>,
}

impl CredentialService {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            password_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Credential cache beállítása - login időpontjában hívandó
    pub async fn set_password(&self, user_id: i64, password: String) {
        let mut cache = self.password_cache.write().await;
        cache.insert(user_id, password);
        tracing::debug!("Password cached for user {}", user_id);
    }

    /// Jelszó törlése - logout időpontjában hívandó
    pub async fn clear_password(&self, user_id: i64) {
        let mut cache = self.password_cache.write().await;
        cache.remove(&user_id);
        let mut cred_cache = self.cache.write().await;
        cred_cache.remove(&user_id);
        tracing::debug!("Password and credentials cleared for user {}", user_id);
    }

    /// Credentialsek lekérése - először cache-ből, ha nincs ott, adatbázisból decryptelve
    pub async fn get_credentials(
        &self,
        db: &Db,
        user_id: i64,
    ) -> Result<PolymarketCredentials, CredentialError> {
        // Először nézzük a cache-t
        {
            let cache = self.cache.read().await;
            if let Some(creds) = cache.get(&user_id) {
                tracing::debug!("Credentials cache hit for user {}", user_id);
                return Ok(creds.clone());
            }
        }

        // Nincs cache-ben, lekérjük az adatbázisból
        let settings = queries::get_settings(db, user_id)
            .await
            .map_err(|e| CredentialError::DatabaseError(e.to_string()))?;

        let (_api_key, encrypted_blob) = match settings {
            Some((key, blob)) if !blob.is_empty() => (key, blob),
            _ => return Err(CredentialError::NotFound),
        };

        // Jelszó lekérése a cache-ből
        let password = {
            let cache = self.password_cache.read().await;
            match cache.get(&user_id).cloned() {
                Some(pw) => pw,
                None => {
                    // Password nincs cache-elve (pl. szerver restart után)
                    // Fall back to legacy api_keys table
                    return Err(CredentialError::PasswordNotCached);
                }
            }
        };

        // Decryptelés
        let encryption_key = format!("{}_pm_creds", password);
        let json_str = crypto::decrypt(&encrypted_blob, &encryption_key)
            .map_err(|e| CredentialError::DecryptError(format!("Decryption failed: {}", e)))?;

        // Parse JSON
        #[derive(serde::Deserialize)]
        struct StoredCreds {
            key: String,
            secret: String,
            passphrase: String,
            #[serde(default)]
            private_key: String,
            #[serde(default)]
            funder: Option<String>,
            #[serde(default)]
            signature_type: Option<u8>,
            #[serde(default)]
            wallet_address: Option<String>,
        }

        let creds: StoredCreds = serde_json::from_str(&json_str)
            .map_err(|e| CredentialError::ParseError(format!("Failed to parse credentials: {}", e)))?;

        // Wallet address vagy a tároltból, vagy újra generáljuk
        let wallet_address = if let Some(addr) = creds.wallet_address.filter(|s| !s.is_empty()) {
            addr
        } else if !creds.private_key.is_empty() {
            // Generáljuk újra a private key-ből
            let client = PolymarketClient::new(&creds.private_key)
                .map_err(|e| CredentialError::InvalidPrivateKey(e.to_string()))?;
            client.address()
        } else {
            return Err(CredentialError::PrivateKeyRequired);
        };

        let result = PolymarketCredentials {
            api_key: creds.key,
            api_secret: creds.secret,
            api_passphrase: creds.passphrase,
            private_key: creds.private_key,
            funder: creds.funder,
            signature_type: creds.signature_type.unwrap_or(0),
            wallet_address,
        };

        // Cache-eljük a jövőbeli kérésekhez
        {
            let mut cache = self.cache.write().await;
            cache.insert(user_id, result.clone());
        }

        Ok(result)
    }

    /// PolymarketClient létrehozása credentialsekből
    pub async fn create_client(
        &self,
        db: &Db,
        user_id: i64,
    ) -> Result<PolymarketClient, CredentialError> {
        let creds = self.get_credentials(db, user_id).await?;

        PolymarketClient::new(&creds.private_key)
            .map_err(|e| CredentialError::InvalidPrivateKey(e.to_string()))
    }

    /// Credential cache törlése (pl. settings update után)
    pub async fn invalidate_cache(&self, user_id: i64) {
        let mut cache = self.cache.write().await;
        cache.remove(&user_id);
        tracing::debug!("Credentials cache invalidated for user {}", user_id);
    }
}

impl Default for CredentialService {
    fn default() -> Self {
        Self::new()
    }
}
