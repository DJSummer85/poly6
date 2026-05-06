use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub market: String,
    pub side: String,        // "buy" or "sell"
    pub price: f64,
    pub size: f64,
    pub order_type: String,  // "GTC" (good till cancel), "FOK", "IOC"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub id: String,
    pub market: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub filled_size: f64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeResponse {
    pub fee_rate_bps: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub balances: Vec<Balance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub available: f64,
    pub locked: f64,
}

pub struct ClobClient {
    base_url: String,
    http_client: Client,
    api_key: Option<String>,
}

impl ClobClient {
    pub fn new(api_key: Option<String>) -> Self {
        let base_url = "https://clob.polymarket.com".to_string();

        Self {
            base_url,
            http_client: Client::new(),
            api_key,
        }
    }

    /// Get current fee rate (required for order signing)
    pub async fn get_fee_rate(&self) -> Result<FeeResponse, String> {
        let url = format!("{}/fee", self.base_url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        response
            .json::<FeeResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get account balance
    pub async fn get_balance(&self) -> Result<BalanceResponse, String> {
        let url = format!("{}/balance", self.base_url);

        let mut request = self.http_client.get(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        response
            .json::<BalanceResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Place an order
    /// Note: In production, orders need to be signed with the user's private key
    /// This is a simplified version that would need proper signature implementation
    pub async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse, String> {
        let url = format!("{}/order", self.base_url);

        let mut request = self.http_client.post(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        // Get current fee rate for the order
        let _fee_rate = self.get_fee_rate().await?;

        // Note: In production, you would:
        // 1. Create order with fee_rate_bps
        // 2. Sign the order with user's private key
        // 3. Submit the signed order

        let response = request
            .json(order)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API error: {} - {}", status, error_text));
        }

        response
            .json::<OrderResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<(), String> {
        let url = format!("{}/order/{}", self.base_url, order_id);

        let mut request = self.http_client.delete(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        Ok(())
    }

    /// Get order status
    pub async fn get_order(&self, order_id: &str) -> Result<OrderResponse, String> {
        let url = format!("{}/order/{}", self.base_url, order_id);

        let mut request = self.http_client.get(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        response
            .json::<OrderResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}
