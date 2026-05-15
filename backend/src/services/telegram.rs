use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::db::Db;
use crate::db::queries;

#[derive(Clone)]
pub struct TelegramService {
    client: Client,
    db: Db,
    cache: Arc<RwLock<HashMap<i64, (String, String)>>>, // user_id -> (token, chat_id)
}

impl TelegramService {
    pub fn new(db: Db) -> Self {
        Self {
            client: Client::new(),
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn send_message(&self, user_id: i64, text: &str) -> Result<(), anyhow::Error> {
        let (token, chat_id) = self.get_creds(user_id).await?;
        
        if token.is_empty() || chat_id.is_empty() {
            tracing::warn!("Telegram notification skipped for user {}: Missing token or chat_id", user_id);
            return Ok(()); // Nem hiba, csak nincs beállítva
        }
        
        tracing::info!("Sending Telegram message to user {}: {}", user_id, text);

        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let resp = self.client.post(url)
            .json(&json!({
                "chat_id": chat_id,
                "text": text,
                "parse_mode": "HTML"
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_text = resp.text().await?;
            tracing::error!("Telegram send failed: {}", err_text);
            return Err(anyhow::anyhow!("Telegram error: {}", err_text));
        }

        Ok(())
    }

    async fn get_creds(&self, user_id: i64) -> Result<(String, String), anyhow::Error> {
        // 1. Check cache
        {
            let cache = self.cache.read().await;
            if let Some(creds) = cache.get(&user_id) {
                return Ok(creds.clone());
            }
        }

        // 2. Load from DB
        let keys = queries::get_api_keys(&self.db, user_id).await?;
        let token = keys.iter().find(|k| k.key_name == "telegram_bot_token").map(|k| k.key_value.clone()).unwrap_or_default();
        let chat_id = keys.iter().find(|k| k.key_name == "telegram_chat_id").map(|k| k.key_value.clone()).unwrap_or_default();

        // 3. Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(user_id, (token.clone(), chat_id.clone()));
        }

        Ok((token, chat_id))
    }

    pub async fn invalidate_cache(&self, user_id: i64) {
        let mut cache = self.cache.write().await;
        cache.remove(&user_id);
    }
}
