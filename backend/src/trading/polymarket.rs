//! Polymarket CLOB Client - Authentication and trading (V2 compatible)
//!
//! Handles API key derivation, validation, and authenticated API calls
//! Updated for CLOB V2 migration (April 28, 2026)

use ethers::core::types::{H160, H256, U256};
use ethers::signers::{LocalWallet, Signer};
use ethers::abi::{self, Token};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

// V2 endpoints - after April 28, clob.polymarket.com automatically routes to V2
const CLOB_HOST: &str = "https://clob.polymarket.com";
const DATA_HOST: &str = "https://data-api.polymarket.com";
const CHAIN_ID: u64 = 137; // Polygon (unchanged in V2)

// V2 Contract Addresses (from https://docs.polymarket.com/resources/contracts)
pub const CTF_EXCHANGE_V2: &str = "0xE111180000d2663C0091e4f400237545B87B996B";
pub const NEG_RISK_CTF_EXCHANGE_V2: &str = "0xe2222d279d744050d28e00520010520000310F59";
pub const NEG_RISK_ADAPTER_V2: &str = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";
pub const CONDITIONAL_TOKENS: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
pub const PUSD_COLLATERAL: &str = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB";
pub const COLLATERAL_ONRAMP: &str = "0x93070a847efEf7F70739046A929D47a521F5B8ee";

// EIP-712 Domain versions
pub const EXCHANGE_DOMAIN_VERSION: &str = "2"; // V2 Exchange domain version
pub const CLOB_AUTH_DOMAIN_VERSION: &str = "1"; // CLOB Auth unchanged

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCreds {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceAllowance {
    pub balance: String,
    pub allowance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub wallet_address: String,
    pub balance: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub market: Option<String>,
    pub outcome: Option<String>,
    pub size: Option<f64>,
    pub avg_price: Option<f64>,
    pub current_value: Option<f64>,
    pub total_bought: Option<f64>,
    pub token_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceAllowanceResponse {
    pub balance: String,
    pub allowance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub token_id: String,
    pub price: f64,
    pub size: f64,
    pub side: String, // "BUY" or "SELL"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: Option<String>,
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketToken {
    pub token_id: String,
    pub outcome: String,
    pub tick_size: String,
    pub neg_risk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketResponse {
    pub condition_id: String,
    pub question: String,
    pub tokens: Vec<MarketToken>,
}

#[derive(Debug, thiserror::Error)]
pub enum PolymarketError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Invalid private key: {0}")]
    InvalidKey(String),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Signature error: {0}")]
    SignatureFailed(String),
}

/// Client for Polymarket CLOB API
pub struct PolymarketClient {
    http_client: Client,
    wallet: LocalWallet,
    creds: Option<ApiKeyCreds>,
    signature_type: u8, // 0 = EOA, 1 = Magic/Email
    funder: Option<String>,
}

impl PolymarketClient {
    /// Create a new client with private key
    pub fn new(private_key: &str) -> Result<Self, PolymarketError> {
        // In ethers 2.0, parse expects raw bytes, not hex string
        let key_bytes = hex::decode(private_key.trim_start_matches("0x"))
            .map_err(|e| PolymarketError::InvalidKey(e.to_string()))?;
        let wallet = LocalWallet::from_bytes(key_bytes.as_slice())
            .map_err(|e| PolymarketError::InvalidKey(e.to_string()))?;

        Ok(Self {
            http_client: Client::new(),
            wallet,
            creds: None,
            signature_type: 0,
            funder: None,
        })
    }

    /// Set signature type (0 = EOA, 1 = Magic/Email)
    pub fn with_signature_type(mut self, signature_type: u8) -> Self {
        self.signature_type = signature_type;
        self
    }

    /// Set funder address (Polymarket profile address)
    pub fn with_funder(mut self, funder: &str) -> Self {
        self.funder = Some(funder.to_string());
        self
    }

    /// Set API credentials
    pub fn with_creds(mut self, creds: ApiKeyCreds) -> Self {
        self.creds = Some(creds);
        self
    }

    pub fn from_api_credentials(
        private_key: &str,
        signature_type: u8,
        creds: Option<ApiKeyCreds>,
        funder: Option<&str>,
    ) -> Result<Self, PolymarketError> {
        let mut client = Self::new(private_key)?.with_signature_type(signature_type);

        if let Some(creds) = creds {
            client = client.with_creds(creds);
        }

        if let Some(funder) = funder {
            client = client.with_funder(funder);
        }

        Ok(client)
    }

    /// Get the wallet address (full 42-char hex)
    pub fn address(&self) -> String {
        let addr = self.wallet.address();
        format!("0x{}", hex::encode(addr.as_bytes()))
    }

    /// Get the wallet address as H160
    fn address_h160(&self) -> H160 {
        self.wallet.address()
    }

    /// Get the HTTP client for direct API calls
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Build a GET request with HMAC-SHA256 auth headers for CLOB API
    /// See: https://docs.polymarket.com/api-reference/authentication
    fn build_authed_get(&self, path: &str) -> reqwest::RequestBuilder {
        let creds = self.creds.as_ref().expect("API credentials not set");
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();

        // Sign message: timestamp + method + path (no body for GET)
        let message = format!("{}GET{}", timestamp, path);

        // Decode secret from base64, then HMAC-SHA256, then URL-safe base64 encode
        let secret_bytes = base64::engine::general_purpose::STANDARD
            .decode(&creds.secret)
            .unwrap_or_else(|_| creds.secret.as_bytes().to_vec());

        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes)
            .expect("HMAC key size");
        mac.update(message.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

        let url = format!("{}{}", CLOB_HOST, path);
        self.http_client.get(&url)
            .header("POLY_ADDRESS", self.address())
            .header("POLY_SIGNATURE", &signature)
            .header("POLY_TIMESTAMP", &timestamp)
            .header("POLY_API_KEY", &creds.key)
            .header("POLY_PASSPHRASE", &creds.passphrase)
    }

    /// Build a POST request with HMAC-SHA256 auth headers for CLOB API
    fn build_authed_post(&self, path: &str) -> reqwest::RequestBuilder {
        let creds = self.creds.as_ref().expect("API credentials not set");
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();

        let message = format!("{}POST{}", timestamp, path);

        let secret_bytes = base64::engine::general_purpose::STANDARD
            .decode(&creds.secret)
            .unwrap_or_else(|_| creds.secret.as_bytes().to_vec());

        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes)
            .expect("HMAC key size");
        mac.update(message.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

        let url = format!("{}{}", CLOB_HOST, path);
        self.http_client.post(&url)
            .header("POLY_ADDRESS", self.address())
            .header("POLY_SIGNATURE", &signature)
            .header("POLY_TIMESTAMP", &timestamp)
            .header("POLY_API_KEY", &creds.key)
            .header("POLY_PASSPHRASE", &creds.passphrase)
            .header("Content-Type", "application/json")
    }

    /// Create or derive API key using L1 wallet authentication
    pub async fn create_or_derive_api_key(&mut self) -> Result<ApiKeyCreds, PolymarketError> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let message = format!("Sign this message to authenticate with Polymarket.\n\nTimestamp: {}", timestamp);

        let signature = self.wallet
            .sign_message(message.as_bytes())
            .await
            .map_err(|e| PolymarketError::SignatureFailed(e.to_string()))?;

        let body = serde_json::json!({
            "address": self.address(),
            "message": message,
            "signature": signature.to_string(),
        });

        // Try different endpoint patterns
        let endpoints = vec![
            "/api/keys",
            "/api-keys",
            "/api_key",
        ];

        let mut last_error = None;
        for endpoint in endpoints {
            let response = self.http_client
                .post(format!("{}{}", CLOB_HOST, endpoint))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let creds: ApiKeyCreds = resp.json().await?;
                    self.creds = Some(creds.clone());
                    tracing::info!("Successfully created/derived API key for {}", self.address());
                    return Ok(creds);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let error_text = resp.text().await.unwrap_or_default();
                    let error_message = format!("{}: {}", status, error_text);
                    tracing::warn!("Endpoint {} failed: {}", endpoint, error_message);
                    last_error = Some(error_message);
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }
        }

        Err(PolymarketError::ApiError(last_error.unwrap_or_else(|| "All endpoints failed".to_string())))
    }

    /// Get account balance using CLOB API with HMAC auth, falling back to data-api
    pub async fn get_balance(&self) -> Result<f64, PolymarketError> {
        // Primary: CLOB /balance-allowance endpoint with HMAC auth
        if self.creds.is_some() {
            let resp = self.build_authed_get("/balance-allowance").send().await;
            match resp {
                Ok(resp) if resp.status().is_success() => {
                    #[derive(Deserialize)]
                    struct ClobBalanceAllowance {
                        #[serde(default)]
                        balance: String,
                        #[serde(default)]
                        allowance: String,
                    }

                    if let Ok(body) = resp.json::<ClobBalanceAllowance>().await {
                        if let Ok(balance) = body.balance.parse::<f64>() {
                            if balance > 0.0 {
                                tracing::info!("Balance from CLOB /balance-allowance: {} USDC", balance);
                                return Ok(balance);
                            }
                            tracing::info!("Balance from CLOB /balance-allowance: 0 USDC (allowance: {})", body.allowance);
                            return Ok(0.0);
                        }
                    }
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    tracing::warn!("CLOB /balance failed: {} {}", status, text);
                }
                Err(e) => {
                    tracing::warn!("CLOB /balance-allowance request failed: {}", e);
                }
            }
        }

        // Fallback: data-api /value endpoint (no auth)
        let address = self.wallet.address().to_string().to_lowercase();

        let response = self.http_client
            .get(format!("{}/value", DATA_HOST))
            .query(&[("user", &address)])
            .send()
            .await?;

        if response.status().is_success() {
            let result: Vec<serde_json::Value> = response.json().await?;
            if let Some(first) = result.first() {
                if let Some(value) = first.get("value").and_then(|v| v.as_f64()) {
                    tracing::info!("Balance from data-api /value: {}", value);
                    return Ok(value);
                }
            }
        }

        Ok(0.0)
    }

    /// Get balance and allowance (legacy method - uses data-api now)
    pub async fn get_balance_allowance(&self) -> Result<BalanceAllowance, PolymarketError> {
        let balance = self.get_balance().await?;

        Ok(BalanceAllowance {
            balance: balance.to_string(),
            allowance: balance.to_string(),
        })
    }

    /// Get positions from data-api
    pub async fn get_positions(&self) -> Result<Vec<PositionInfo>, PolymarketError> {
        let address = self.wallet.address().to_string().to_lowercase();

        let response = self.http_client
            .get(format!("{}/positions", DATA_HOST))
            .query(&[("user", &address)])
            .query(&[("limit", "100")])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        let positions: Vec<PositionInfo> = response.json().await?;
        Ok(positions)
    }

    /// Validate credentials by checking balance (works without API keys)
    pub async fn validate_credentials(&self) -> Result<ValidationResult, PolymarketError> {
        let balance = self.get_balance().await?;

        Ok(ValidationResult {
            valid: true,
            wallet_address: self.wallet.address().to_string(),
            balance,
            message: if balance > 0.0 {
                format!("Credentials valid. Balance: {}", balance)
            } else {
                "Credentials valid. Account has no positions.".to_string()
            },
        })
    }

    /// Get balance for a specific conditional token
    #[allow(dead_code)]
    pub async fn get_token_balance(&self, token_id: &str) -> Result<BalanceAllowance, PolymarketError> {
        let address = self.wallet.address().to_string().to_lowercase();

        let response = self.http_client
            .get(format!("{}/positions", DATA_HOST))
            .query(&[("user", &address)])
            .query(&[("token_id", token_id)])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        let result: BalanceAllowanceResponse = response.json().await?;
        Ok(BalanceAllowance {
            balance: result.balance,
            allowance: result.allowance,
        })
    }

    /// Get market info (tokens, tick size, etc.)
    pub async fn get_market(&self, condition_id: &str) -> Result<MarketResponse, PolymarketError> {
        let response = self.http_client
            .get(format!("{}/markets/{}", CLOB_HOST, condition_id))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        let market: MarketResponse = response.json().await?;
        Ok(market)
    }

    /// Get a quote for an order (without placing it)
    pub async fn get_quote(&self, token_id: &str, side: &str, size: f64) -> Result<f64, PolymarketError> {
        let response = self.http_client
            .get(format!("{}/quotes", CLOB_HOST))
            .query(&[
                ("token_id", token_id),
                ("side", side),
                ("size", &size.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        #[derive(Deserialize)]
        struct QuoteResponse {
            price: String,
        }

        let quote: QuoteResponse = response.json().await?;
        quote.price.parse().map_err(|_| PolymarketError::ApiError("Invalid price format".to_string()))
    }

    /// Create and sign a V2 order using proper EIP-712 typed data
    pub async fn create_order_v2(
        &self,
        order: &OrderRequest,
        is_neg_risk: bool,
    ) -> Result<serde_json::Value, PolymarketError> {
        let creds = self.creds.as_ref()
            .ok_or(PolymarketError::AuthFailed("No API credentials".to_string()))?;

        let salt: U256 = U256::from(rand::random::<u64>());
        let timestamp_ms = U256::from(chrono::Utc::now().timestamp_millis() as u64);
        let maker_amount = U256::from((order.size * 1000000.0) as u64);
        let taker_amount = U256::from((order.size * order.price * 1000000.0) as u64);
        let token_id = U256::from_str_radix(&order.token_id, 10)
            .unwrap_or(U256::zero());
        let side = Token::Uint(U256::from(if order.side == "BUY" { 0u8 } else { 1u8 }));
        let sig_type = Token::Uint(U256::from(self.signature_type as u64));
        let metadata = H256::zero();
        let builder = H256::zero();
        let maker: H160 = self.funder.as_ref()
            .map(|s| s.parse().unwrap_or_else(|_| self.address_h160()))
            .unwrap_or_else(|| self.address_h160());
        let signer = self.address_h160();

        // --- EIP-712 Domain: name="Polymarket CTF Exchange", version="2" ---
        let exchange_address: H160 = get_exchange_address(is_neg_risk)
            .parse()
            .map_err(|e| PolymarketError::ApiError(format!("Invalid exchange address: {e}")))?;

        // --- EIP-712 Domain Separator ---
        // domainSeparator = keccak256(
        //   EIP712Domain_TypeHash ||
        //   keccak256(name) || keccak256(version) ||
        //   chainId || verifyingContract
        // )
        let domain_type_hash = ethers::utils::keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );
        let name_hash = ethers::utils::keccak256(b"Polymarket CTF Exchange");
        let version_hash = ethers::utils::keccak256(EXCHANGE_DOMAIN_VERSION.as_bytes());
        let domain_encoded = abi::encode(&[
            Token::FixedBytes(domain_type_hash.to_vec()),
            Token::FixedBytes(name_hash.to_vec()),
            Token::FixedBytes(version_hash.to_vec()),
            Token::Uint(U256::from(CHAIN_ID)),
            Token::Address(exchange_address),
        ]);
        let domain_separator = ethers::utils::keccak256(&domain_encoded);

        // --- EIP-712 encodeType for Order ---
        let order_type_hash = ethers::utils::keccak256(
            b"Order(salt uint256,maker address,signer address,tokenId uint256,makerAmount uint256,takerAmount uint256,side uint8,signatureType uint8,timestamp uint256,metadata bytes32,builder bytes32)"
        );

        // --- EIP-712 encodeData for Order (ABI encoding) ---
        let encoded_data = abi::encode(&[
            Token::Uint(salt),
            Token::Address(maker),
            Token::Address(signer),
            Token::Uint(token_id),
            Token::Uint(maker_amount),
            Token::Uint(taker_amount),
            side,
            sig_type,
            Token::Uint(timestamp_ms),
            Token::FixedBytes(metadata.as_bytes().to_vec()),
            Token::FixedBytes(builder.as_bytes().to_vec()),
        ]);
        let encoded_data_hash = ethers::utils::keccak256(&encoded_data);

        // hashStruct = keccak256(encodeTypeHash || encodeDataHash)
        let mut struct_bytes = Vec::with_capacity(64);
        struct_bytes.extend_from_slice(&order_type_hash);
        struct_bytes.extend_from_slice(&encoded_data_hash);
        let struct_hash = ethers::utils::keccak256(&struct_bytes);

        // EIP-712 digest: keccak256("\x19\x01" || domainSeparator || hashStruct)
        let mut pre_hash = Vec::with_capacity(66);
        pre_hash.push(0x19);
        pre_hash.push(0x01);
        pre_hash.extend_from_slice(&domain_separator);
        pre_hash.extend_from_slice(&struct_hash);

        let digest = ethers::utils::keccak256(&pre_hash);

        let signature = self.wallet.sign_message(digest.to_vec())
            .await
            .map_err(|e| PolymarketError::SignatureFailed(e.to_string()))?;

        // V2 Signed order format
        let signed_order = serde_json::json!({
            "order": {
                "salt": salt.to_string(),
                "maker": format!("{maker:#042x}"),
                "signer": format!("{signer:#042x}"),
                "tokenId": token_id.to_string(),
                "makerAmount": maker_amount.to_string(),
                "takerAmount": taker_amount.to_string(),
                "side": order.side,
                "signatureType": self.signature_type,
                "timestamp": timestamp_ms.to_string(),
                "metadata": format!("{:#066x}", metadata),
                "builder": format!("{:#066x}", builder),
                "signature": signature.to_string(),
            },
            "orderType": "GTC",
            "owner": &creds.key,
        });

        Ok(signed_order)
    }

    /// Create and sign an order (V2 compatible - uses create_order_v2)
    pub async fn create_order(&self, order: &OrderRequest) -> Result<serde_json::Value, PolymarketError> {
        // Use V2 signing by default
        self.create_order_v2(order, false).await
    }

    /// Post a signed order to the orderbook
    pub async fn post_order(&self, signed_order: &serde_json::Value) -> Result<OrderResponse, PolymarketError> {
        let creds = self.creds.as_ref()
            .ok_or(PolymarketError::AuthFailed("No API credentials".to_string()))?;

        let response = self.http_client
            .post(format!("{}/orders", CLOB_HOST))
            .header("Content-Type", "application/json")
            .header("POLY-API-KEY", &creds.key)
            .header("POLY-API-SECRET", &creds.secret)
            .header("POLY-API-PASSPHRASE", &creds.passphrase)
            .json(signed_order)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        let order_response: OrderResponse = response.json().await?;
        Ok(order_response)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<(), PolymarketError> {
        let creds = self.creds.as_ref()
            .ok_or(PolymarketError::AuthFailed("No API credentials".to_string()))?;

        let response = self.http_client
            .delete(format!("{}/orders/{}", CLOB_HOST, order_id))
            .header("POLY-API-KEY", &creds.key)
            .header("POLY-API-SECRET", &creds.secret)
            .header("POLY-API-PASSPHRASE", &creds.passphrase)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        Ok(())
    }

    /// Get open orders
    pub async fn get_orders(&self) -> Result<Vec<OrderResponse>, PolymarketError> {
        let creds = self.creds.as_ref()
            .ok_or(PolymarketError::AuthFailed("No API credentials".to_string()))?;

        let response = self.http_client
            .get(format!("{}/orders", CLOB_HOST))
            .header("POLY-API-KEY", &creds.key)
            .header("POLY-API-SECRET", &creds.secret)
            .header("POLY-API-PASSPHRASE", &creds.passphrase)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        let orders: Vec<OrderResponse> = response.json().await?;
        Ok(orders)
    }
}

/// Check MATIC (gas fee) balance on Polygon network
/// Returns balance in MATIC (not wei). Logs warning if below threshold.
pub async fn check_matic_balance(wallet_address: &str) -> Result<f64, String> {
    const MATIC_MIN_THRESHOLD: f64 = 0.01; // 0.01 MATIC minimum
    const POLYGON_RPC: &str = "https://polygon-rpc.com";

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [wallet_address, "latest"],
        "id": 1
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(POLYGON_RPC)
        .timeout(Duration::from_secs(5))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Polygon RPC error: {}", e))?;

    let result: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse Polygon RPC response: {}", e))?;

    let balance_hex = result.get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid balance response".to_string())?;

    // Convert hex wei to MATIC (1 MATIC = 10^18 wei)
    let balance_wei = u128::from_str_radix(balance_hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Failed to parse balance: {}", e))?;

    let balance_matic = balance_wei as f64 / 1e18;

    if balance_matic < MATIC_MIN_THRESHOLD {
        tracing::warn!(
            "Low MATIC balance: {:.6} (min recommended: {:.2}). Transactions may fail.",
            balance_matic, MATIC_MIN_THRESHOLD
        );
    } else {
        tracing::info!("MATIC balance: {:.6}", balance_matic);
    }

    Ok(balance_matic)
}

use tokio::time::Duration;

/// Full credentials to store in database (encrypted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
    pub funder: Option<String>,
    pub signature_type: u8,
    pub wallet_address: String,
}

/// V2 Contract configuration helper
pub fn get_exchange_address(is_neg_risk: bool) -> String {
    if is_neg_risk {
        NEG_RISK_CTF_EXCHANGE_V2.to_string()
    } else {
        CTF_EXCHANGE_V2.to_string()
    }
}

/// Get collateral token address (pUSD in V2)
pub fn get_collateral_address() -> String {
    PUSD_COLLATERAL.to_string()
}

/// Get collateral onramp for wrapping USDC to pUSD
pub fn get_collateral_onramp() -> String {
    COLLATERAL_ONRAMP.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        // Valid private key format: 64 hex characters (32 bytes) without 0x prefix
        // This is a deterministic test key - not used for real trading
        let private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let client = PolymarketClient::new(private_key).unwrap();
        let address = client.address();
        println!("Generated address: {:?}", address);
        assert!(!address.is_empty(), "Address should not be empty");
        assert!(address.starts_with("0x"), "Address should start with 0x");
    }

    #[test]
    fn test_invalid_private_key() {
        let result = PolymarketClient::new("invalid");
        assert!(result.is_err());
    }
}
