//! Polymarket CLOB Client - Authentication and trading (V2 compatible)
//!
//! Port of @polymarket/clob-client-v2 Python SDK to Rust.
//! See: https://github.com/Polymarket/py-clob-client-v2

use ethers::core::types::{H160, U256};
use ethers::signers::{LocalWallet, Signer};
use ethers::abi::{self, Token};
use k256::elliptic_curve::generic_array::GenericArray;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine, engine::general_purpose::{URL_SAFE_NO_PAD, STANDARD}};
use std::time::Duration;

const CLOB_HOST: &str = "https://clob.polymarket.com";
const RELAYER_HOST: &str = "https://relayer-v2.polymarket.com";
const DATA_HOST: &str = "https://data-api.polymarket.com";
const CHAIN_ID: u64 = 137;

// Exchange addresses from official Polymarket clob-client config.ts (Polygon/MATIC)
// V2 exchange addresses (post-April 2026 migration)
pub const CTF_EXCHANGE_V2: &str = "0xE111180000d2663C0091e4f400237545B87B996B";
pub const NEG_RISK_CTF_EXCHANGE_V2: &str = "0xe2222d279d744050d28e00520010520000310F59";
pub const NEG_RISK_ADAPTER_V2: &str = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";
pub const CONDITIONAL_TOKENS: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
pub const PUSD_COLLATERAL: &str = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB";
pub const COLLATERAL_ONRAMP: &str = "0x93070a847efEf7F70739046A929D47a521F5B8ee";

pub const USDC_E_TOKEN: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
pub const USDC_NATIVE: &str = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359";
pub const POLYGON_RPCS: &[&str] = &[
    "https://rpc.ankr.com/polygon",
    "https://polygon-mainnet.public.blastapi.io",
    "https://1rpc.io/matic",
    "https://polygon.llamarpc.com",
];

pub const POLYGON_RPC: &str = "https://rpc.ankr.com/polygon";
pub const EXCHANGE_DOMAIN_VERSION: &str = "2";
pub const CLOB_AUTH_DOMAIN_VERSION: &str = "1";

// Deposit wallet factory address (Polygon Mainnet)
pub const DEPOSIT_WALLET_FACTORY: &str = "0x00000000000Fb5C9ADea0298D729A0CB3823Cc07";

// ─── EIP-712 Type Strings (V2) ───────────────────────────────────────────────
// These MUST match the Python SDK exactly.
// Note: NO expiration, taker, nonce, or feeRateBps in the struct!

const ORDER_TYPE_STRING: &str = concat!(
    "Order(uint256 salt,address maker,address signer,uint256 tokenId,",
    "uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,",
    "uint256 timestamp,bytes32 metadata,bytes32 builder)"
);

const SOLADY_TYPE_STRING: &str = concat!(
    "TypedDataSign(Order contents,string name,string version,uint256 chainId,",
    "address verifyingContract,bytes32 salt)",
    "Order(uint256 salt,address maker,address signer,uint256 tokenId,",
    "uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,",
    "uint256 timestamp,bytes32 metadata,bytes32 builder)"
);

const DOMAIN_TYPE_STRING: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

// ─── Structs ─────────────────────────────────────────────────────────────────

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
    pub side: String,
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

pub struct PolymarketClient {
    http_client: Client,
    wallet: LocalWallet,
    private_key_bytes: Vec<u8>,
    creds: Option<ApiKeyCreds>,
    signature_type: u8,
    funder: Option<String>,
    // Pre-computed exchange domain separator (CTF Exchange V2)
    app_domain_separator: [u8; 32],
    // Deposit wallet address (if using POLY_1271 signing)
    deposit_wallet_address: Option<H160>,
}

impl PolymarketClient {
    pub fn new(private_key: &str) -> Result<Self, PolymarketError> {
        let key_bytes = hex::decode(private_key.trim_start_matches("0x"))
            .map_err(|e| PolymarketError::InvalidKey(e.to_string()))?;
        let wallet = LocalWallet::from_bytes(key_bytes.as_slice())
            .map_err(|e| PolymarketError::InvalidKey(e.to_string()))?;

        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| PolymarketError::RequestFailed(e))?;

        // Pre-compute the CTF Exchange domain separator
        let app_domain_separator = Self::compute_app_domain_separator(None);

        Ok(Self {
            http_client,
            wallet,
            private_key_bytes: key_bytes,
            creds: None,
            signature_type: 0,
            funder: None,
            app_domain_separator,
            deposit_wallet_address: None,
        })
    }

    pub fn with_signature_type(mut self, signature_type: u8) -> Self {
        self.signature_type = signature_type;
        self
    }

    pub fn with_funder(mut self, funder: &str) -> Self {
        self.funder = Some(funder.to_string());
        self
    }

    pub fn with_creds(mut self, creds: ApiKeyCreds) -> Self {
        self.creds = Some(creds);
        self
    }

    /// Set the deposit wallet address for POLY_1271 signing.
    /// When set, orders will use signature_type=3 and ERC-7739 wrapping.
    pub fn with_deposit_wallet(mut self, address: H160) -> Self {
        self.deposit_wallet_address = Some(address);
        self
    }

    pub fn from_api_credentials(
        private_key: &str,
        signature_type: u8,
        creds: Option<ApiKeyCreds>,
        funder: Option<&str>,
        deposit_wallet_address: Option<&str>,
    ) -> Result<Self, PolymarketError> {
        let mut client = Self::new(private_key)?.with_signature_type(signature_type);
        if let Some(creds) = creds {
            client = client.with_creds(creds);
        }
        if let Some(funder) = funder {
            client = client.with_funder(funder);
        }
    if let Some(deposit_addr) = deposit_wallet_address {
        if !deposit_addr.is_empty() {
            // H160::from_str in fixed-hash 0.8 does NOT handle 0x prefix (returns error)
            // Strip it first and ensure lowercase
            let hex_str = deposit_addr.strip_prefix("0x").unwrap_or(deposit_addr).to_lowercase();
            tracing::debug!("[DEPOSIT_WALLET_DEBUG] original='{}' stripped='{}' len={}", deposit_addr, hex_str, hex_str.len());
            match hex_str.parse::<H160>() {
                Ok(addr) => {
                    client = client.with_deposit_wallet(addr);
                    tracing::info!("Deposit wallet set on client: {:?}", addr);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse deposit wallet '{}' (len={}): {} - {}", hex_str, hex_str.len(), e, deposit_addr);
                }
            }
        } else {
            tracing::warn!("deposit_wallet_address is Some but empty string");
        }
    } else {
        tracing::debug!("deposit_wallet_address parameter is None");
    }
        Ok(client)
    }

    pub fn address(&self) -> String {
        format!("0x{}", hex::encode(self.wallet.address().as_bytes()))
    }

    /// Returns the EOA wallet address for POLY_ADDRESS header in HMAC auth.
    /// Must match the address the API key is registered to.
    fn hmac_address(&self) -> String {
        self.address()
    }

    fn address_h160(&self) -> H160 {
        self.wallet.address()
    }

    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    // ─── EIP-712 Helpers ─────────────────────────────────────────────────

    /// Compute the CTF Exchange V2 domain separator.
    fn compute_app_domain_separator(neg_risk_exchange: Option<&str>) -> [u8; 32] {
        let exchange = neg_risk_exchange.unwrap_or(CTF_EXCHANGE_V2);
        let exchange_addr: H160 = exchange.parse().expect("Invalid exchange address");

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
            Token::Address(exchange_addr),
        ]);

        ethers::utils::keccak256(&domain_encoded)
    }

    /// Compute the Deposit Wallet domain separator.
    /// Domain: EIP712Domain(name="DepositWallet", version="1", chainId=137, verifyingContract=deposit_wallet_addr)
    fn compute_deposit_wallet_domain_separator(deposit_wallet_addr: H160) -> [u8; 32] {
        let domain_type_hash = ethers::utils::keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );
        let name_hash = ethers::utils::keccak256(b"DepositWallet");
        let version_hash = ethers::utils::keccak256(b"1");

        let domain_encoded = abi::encode(&[
            Token::FixedBytes(domain_type_hash.to_vec()),
            Token::FixedBytes(name_hash.to_vec()),
            Token::FixedBytes(version_hash.to_vec()),
            Token::Uint(U256::from(CHAIN_ID)),
            Token::Address(deposit_wallet_addr),
        ]);

        ethers::utils::keccak256(&domain_encoded)
    }

    /// Compute the EIP-712 digest for a standard EOA order.
    /// digest = keccak256(0x19 || 0x01 || domainSeparator || structHash)
    fn compute_eip712_digest(domain_separator: &[u8; 32], struct_hash: &[u8; 32]) -> [u8; 32] {
        let mut pre_hash = Vec::with_capacity(66);
        pre_hash.push(0x19);
        pre_hash.push(0x01);
        pre_hash.extend_from_slice(domain_separator);
        pre_hash.extend_from_slice(struct_hash);
        ethers::utils::keccak256(&pre_hash)
    }

    /// Sign a 32-byte digest with raw ECDSA (no prefix).
    /// Returns 65-byte signature (r || s || v) as hex string with 0x prefix.
    fn sign_digest(private_key_bytes: &[u8], digest: &[u8; 32]) -> Result<String, PolymarketError> {
        use k256::ecdsa::SigningKey;

        let key_array = GenericArray::from_slice(private_key_bytes);
        let signing_key = SigningKey::from_bytes(key_array)
            .map_err(|e| PolymarketError::SignatureFailed(format!("Invalid signing key: {e}")))?;

        let (sig_bytes, rec_id) = signing_key
            .sign_prehash_recoverable(digest)
            .map_err(|e| PolymarketError::SignatureFailed(format!("Signing failed: {e}")))?;

        let (r_bytes, s_bytes) = sig_bytes.split_bytes();

        let mut sig_hex = String::with_capacity(132);
        sig_hex.push_str("0x");
        sig_hex.push_str(&hex::encode(r_bytes.as_slice()));
        sig_hex.push_str(&hex::encode(s_bytes.as_slice()));
        sig_hex.push_str(&hex::encode(&[rec_id.to_byte() + 27]));

        Ok(sig_hex)
    }

    /// Compute the Order struct hash (the inner contents_hash for POLY_1271).
    /// This is keccak256(typeHash || abi.encode(order_fields...)).
    /// Matches the Python SDK's contents_hash computation.
    fn compute_contents_hash(
        salt: &U256,
        maker: &H160,
        signer: &H160,
        token_id: &U256,
        maker_amount: &U256,
        taker_amount: &U256,
        side: u8,
        signature_type: u8,
        timestamp: &U256,
        metadata: &[u8; 32],
        builder: &[u8; 32],
    ) -> [u8; 32] {
        let order_type_hash = ethers::utils::keccak256(ORDER_TYPE_STRING.as_bytes());

        let encoded_fields = abi::encode(&[
            Token::FixedBytes(order_type_hash.to_vec()),
            Token::Uint(*salt),
            Token::Address(*maker),
            Token::Address(*signer),
            Token::Uint(*token_id),
            Token::Uint(*maker_amount),
            Token::Uint(*taker_amount),
            Token::Uint(U256::from(side as u64)),
            Token::Uint(U256::from(signature_type as u64)),
            Token::Uint(*timestamp),
            Token::FixedBytes(metadata.to_vec()),
            Token::FixedBytes(builder.to_vec()),
        ]);

        ethers::utils::keccak256(&encoded_fields)
    }

    /// Compute the Solady TypedDataSign struct hash.
    /// This is the struct hash for the ERC-7739 wrapped signature.
    /// Matches the Python SDK's typed_data_sign_struct_hash computation.
    fn compute_solady_struct_hash(
        contents_hash: &[u8; 32],
        chain_id: u64,
        deposit_wallet_addr: &H160,
    ) -> [u8; 32] {
        let solady_type_hash = ethers::utils::keccak256(SOLADY_TYPE_STRING.as_bytes());
        let wallet_name_hash = ethers::utils::keccak256(b"DepositWallet");
        let wallet_version_hash = ethers::utils::keccak256(b"1");
        let domain_salt = [0u8; 32]; // DEPOSIT_WALLET_DOMAIN_SALT = bytes32(0)

        let encoded = abi::encode(&[
            Token::FixedBytes(solady_type_hash.to_vec()),
            Token::FixedBytes(contents_hash.to_vec()),
            Token::FixedBytes(wallet_name_hash.to_vec()),
            Token::FixedBytes(wallet_version_hash.to_vec()),
            Token::Uint(U256::from(chain_id)),
            Token::Address(*deposit_wallet_addr),
            Token::FixedBytes(domain_salt.to_vec()),
        ]);

        ethers::utils::keccak256(&encoded)
    }

    /// Build an ERC-7739 (POLY_1271) wrapped signature.
    ///
    /// Matches the Python SDK's `_build_poly_1271_order_signature`.
    /// Returns: "0x" + inner_sig (65B) + app_domain_separator (32B) + contents_hash (32B) + contents_type (hex) + type_len (2B big-endian hex)
    fn build_poly_1271_signature(
        private_key_bytes: &[u8],
        app_domain_separator: &[u8; 32],
        contents_hash: &[u8; 32],
        chain_id: u64,
        deposit_wallet_addr: &H160,
    ) -> Result<String, PolymarketError> {
        // Step 1: Compute the Solady TypedDataSign struct hash
        let solady_struct_hash =
            Self::compute_solady_struct_hash(contents_hash, chain_id, deposit_wallet_addr);

        // Step 2: Compute the digest
        // digest = keccak256(0x19 || 0x01 || app_domain_separator || typed_data_sign_struct_hash)
        let digest = Self::compute_eip712_digest(app_domain_separator, &solady_struct_hash);

        // Step 3: Sign with raw ECDSA — get 65-byte inner signature
        let inner_sig_hex = Self::sign_digest(private_key_bytes, &digest)?;
        let inner_sig_hex = inner_sig_hex.trim_start_matches("0x");

        // Step 4: Compute the contents type string (ORDER_TYPE_STRING encoded as hex)
        let contents_type_hex = hex::encode(ORDER_TYPE_STRING.as_bytes());
        let type_len = ORDER_TYPE_STRING.len();
        let type_len_bytes = (type_len as u16).to_be_bytes();
        let type_len_hex = hex::encode(type_len_bytes);

        // Step 5: Assemble the final signature
        // "0x" + inner_sig + app_domain_separator + contents_hash + contents_type + type_len
        let mut signature = String::with_capacity(
            2 + 130 + 64 + 64 + contents_type_hex.len() + type_len_hex.len(),
        );
        signature.push_str("0x");
        signature.push_str(inner_sig_hex);
        signature.push_str(&hex::encode(app_domain_separator));
        signature.push_str(&hex::encode(contents_hash));
        signature.push_str(&contents_type_hex);
        signature.push_str(&type_len_hex);

        Ok(signature)
    }

    // ─── HMAC Authentication ──────────────────────────────────────────────

    fn decode_api_secret(secret: &str) -> Vec<u8> {
        let normalized = secret.replace("-", "+").replace("_", "/");
        let padded = match normalized.len() % 4 {
            0 => normalized,
            r => format!("{}{}", normalized, "=".repeat(4 - r)),
        };
        base64::engine::general_purpose::STANDARD
            .decode(&padded)
            .unwrap_or_else(|_| secret.as_bytes().to_vec())
    }

    fn build_authed_get(&self, path: &str) -> reqwest::RequestBuilder {
        let creds = self.creds.as_ref().expect("API credentials not set");
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let message = format!("{}GET{}", timestamp, path);

        let secret_bytes = Self::decode_api_secret(&creds.secret);

        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).expect("HMAC key size");
        mac.update(message.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());

        let url = format!("{}{}", CLOB_HOST, path);
        self.http_client.get(&url)
            .header("POLY_ADDRESS", self.hmac_address())
            .header("POLY_SIGNATURE", &signature)
            .header("POLY_TIMESTAMP", &timestamp)
            .header("POLY_API_KEY", &creds.key)
            .header("POLY_PASSPHRASE", &creds.passphrase)
    }

    fn build_relayer_get(&self, path: &str) -> reqwest::RequestBuilder {
        let creds = self.creds.as_ref().expect("API credentials not set");
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let message = format!("{}GET{}", timestamp, path);

        let secret_bytes = Self::decode_api_secret(&creds.secret);

        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).expect("HMAC key size");
        mac.update(message.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());

        let url = format!("{}{}", RELAYER_HOST, path);
        self.http_client.get(&url)
            .header("POLY_ADDRESS", self.hmac_address())
            .header("POLY_SIGNATURE", &signature)
            .header("POLY_TIMESTAMP", &timestamp)
            .header("POLY_API_KEY", &creds.key)
            .header("POLY_PASSPHRASE", &creds.passphrase)
    }

    fn build_authed_post_with_body(&self, path: &str, body: &str) -> reqwest::RequestBuilder {
        let creds = self.creds.as_ref().expect("API credentials not set");
        let timestamp = chrono::Utc::now().timestamp().to_string();

        let message = format!("{}POST{}{}", timestamp, path, body);

        let secret_bytes = Self::decode_api_secret(&creds.secret);

        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).expect("HMAC key size");
        mac.update(message.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());

        let url = format!("{}{}", CLOB_HOST, path);
        self.http_client.post(&url)
            .header("POLY_ADDRESS", self.hmac_address())
            .header("POLY_SIGNATURE", &signature)
            .header("POLY_TIMESTAMP", &timestamp)
            .header("POLY_API_KEY", &creds.key)
            .header("POLY_PASSPHRASE", &creds.passphrase)
            .header("Content-Type", "application/json")
            .body(body.to_string())
    }

    // ─── ClobAuth L1 Authentication ───────────────────────────────────────

    fn compute_clob_auth_signature(
        private_key_bytes: &[u8],
        address: H160,
        timestamp_str: &str,
        nonce: U256,
        message_str: &str,
    ) -> Result<String, PolymarketError> {
        let domain_type_hash = ethers::utils::keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId)"
        );
        let name_hash = ethers::utils::keccak256(b"ClobAuthDomain");
        let version_hash = ethers::utils::keccak256(CLOB_AUTH_DOMAIN_VERSION.as_bytes());

        let domain_encoded = abi::encode(&[
            Token::FixedBytes(domain_type_hash.to_vec()),
            Token::FixedBytes(name_hash.to_vec()),
            Token::FixedBytes(version_hash.to_vec()),
            Token::Uint(U256::from(CHAIN_ID)),
        ]);
        let domain_separator = ethers::utils::keccak256(&domain_encoded);

        let type_hash = ethers::utils::keccak256(
            b"ClobAuth(address address,string timestamp,uint256 nonce,string message)"
        );

        let timestamp_hash = ethers::utils::keccak256(timestamp_str.as_bytes());
        let message_hash = ethers::utils::keccak256(message_str.as_bytes());

        let encoded_fields = abi::encode(&[
            Token::Address(address),
            Token::FixedBytes(timestamp_hash.to_vec()),
            Token::Uint(nonce),
            Token::FixedBytes(message_hash.to_vec()),
        ]);

        let mut struct_bytes = Vec::with_capacity(32 + encoded_fields.len());
        struct_bytes.extend_from_slice(&type_hash);
        struct_bytes.extend_from_slice(&encoded_fields);
        let struct_hash = ethers::utils::keccak256(&struct_bytes);

        let digest = Self::compute_eip712_digest(&domain_separator, &struct_hash);
        Self::sign_digest(private_key_bytes, &digest)
    }

    fn parse_api_key_response(raw: &serde_json::Value) -> Result<ApiKeyCreds, PolymarketError> {
        let key = raw.get("apiKey")
            .or_else(|| raw.get("key"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| PolymarketError::ApiError("Missing apiKey/key in response".into()))?
            .to_string();
        let secret = raw.get("secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PolymarketError::ApiError("Missing secret in response".into()))?
            .to_string();
        let passphrase = raw.get("passphrase")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PolymarketError::ApiError("Missing passphrase in response".into()))?
            .to_string();

        Ok(ApiKeyCreds { key, secret, passphrase })
    }

    // ─── API Key Management ───────────────────────────────────────────────

    pub async fn create_or_derive_api_key(&mut self) -> Result<ApiKeyCreds, PolymarketError> {
        let address = self.address_h160();
        let address_str = format!("{:#042x}", address);
        let message_str = "This message attests that I control the given wallet";

        // Attempt 1: POST /auth/api-key (create new) with nonce=0
        {
            let ts = chrono::Utc::now().timestamp().to_string();
            let nonce = U256::zero();
            let sig = Self::compute_clob_auth_signature(
                &self.private_key_bytes, address, &ts, nonce, message_str,
            )?;

            let body = serde_json::json!({});
            let url = format!("{}/auth/api-key", CLOB_HOST);
            let resp = self.http_client
                .post(&url)
                .header("POLY_ADDRESS", &address_str)
                .header("POLY_SIGNATURE", &sig)
                .header("POLY_TIMESTAMP", &ts)
                .header("POLY_NONCE", "0")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(raw) = r.json::<serde_json::Value>().await {
                        if let Ok(creds) = Self::parse_api_key_response(&raw) {
                            self.creds = Some(creds.clone());
                            tracing::info!("Created new API key via POST /auth/api-key");
                            return Ok(creds);
                        }
                    }
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    tracing::warn!("POST /auth/api-key (nonce=0) failed: {}: {}", status, text);
                }
                Err(e) => {
                    tracing::warn!("POST /auth/api-key (nonce=0) error: {}", e);
                }
            }
        }

        // Attempt 2: GET /auth/derive-api-key (derive existing) with nonce=0
        {
            let ts = chrono::Utc::now().timestamp().to_string();
            let nonce = U256::zero();
            let sig = Self::compute_clob_auth_signature(
                &self.private_key_bytes, address, &ts, nonce, message_str,
            )?;

            let url = format!("{}/auth/derive-api-key", CLOB_HOST);
            let resp = self.http_client
                .get(&url)
                .header("POLY_ADDRESS", &address_str)
                .header("POLY_SIGNATURE", &sig)
                .header("POLY_TIMESTAMP", &ts)
                .header("POLY_NONCE", "0")
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(raw) = r.json::<serde_json::Value>().await {
                        if let Ok(creds) = Self::parse_api_key_response(&raw) {
                            self.creds = Some(creds.clone());
                            tracing::info!("Derived existing API key via GET /auth/derive-api-key");
                            return Ok(creds);
                        }
                    }
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    tracing::warn!("GET /auth/derive-api-key failed: {}: {}", status, text);
                }
                Err(e) => {
                    tracing::warn!("GET /auth/derive-api-key error: {}", e);
                }
            }
        }

        // Attempt 3: POST /auth/api-key with random nonce (create fresh)
        {
            let ts = chrono::Utc::now().timestamp().to_string();
            let random_nonce = U256::from(rand::random::<u32>());
            let sig = Self::compute_clob_auth_signature(
                &self.private_key_bytes, address, &ts, random_nonce, message_str,
            )?;

            let body = serde_json::json!({});
            let url = format!("{}/auth/api-key", CLOB_HOST);
            let resp = self.http_client
                .post(&url)
                .header("POLY_ADDRESS", &address_str)
                .header("POLY_SIGNATURE", &sig)
                .header("POLY_TIMESTAMP", &ts)
                .header("POLY_NONCE", random_nonce.to_string())
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(raw) = r.json::<serde_json::Value>().await {
                        if let Ok(creds) = Self::parse_api_key_response(&raw) {
                            self.creds = Some(creds.clone());
                            tracing::info!("Created new API key via POST /auth/api-key (random nonce)");
                            return Ok(creds);
                        }
                    }
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    tracing::warn!("POST /auth/api-key (random nonce) failed: {}: {}", status, text);
                }
                Err(e) => {
                    tracing::warn!("POST /auth/api-key (random nonce) error: {}", e);
                }
            }
        }

        // Attempt 4 (fallback): old endpoints with old authentication
        {
            let old_timestamp = chrono::Utc::now().timestamp_millis();
            let old_message = format!("Sign this message to authenticate with Polymarket.\n\nTimestamp: {}", old_timestamp);
            let old_signature = self.wallet.sign_message(old_message.as_bytes()).await
                .map_err(|e| PolymarketError::SignatureFailed(e.to_string()))?;
            let old_body = serde_json::json!({
                "address": self.address(),
                "message": old_message,
                "signature": old_signature.to_string()
            });

            for endpoint in &["/api/keys", "/api-keys", "/api_key"] {
                let response = self.http_client.post(format!("{}{}", CLOB_HOST, endpoint))
                    .header("Content-Type", "application/json")
                    .json(&old_body)
                    .send()
                    .await;
                match response {
                    Ok(r) if r.status().is_success() => {
                        if let Ok(raw) = r.json::<serde_json::Value>().await {
                            if let Ok(creds) = Self::parse_api_key_response(&raw) {
                                self.creds = Some(creds.clone());
                                tracing::info!("Created API key via fallback endpoint {}", endpoint);
                                return Ok(creds);
                            }
                        }
                    }
                    Ok(r) => {
                        let status = r.status();
                        let text = r.text().await.unwrap_or_default();
                        tracing::warn!("Fallback {} failed: {}: {}", endpoint, status, text);
                    }
                    Err(e) => {
                        tracing::warn!("Fallback {} error: {}", endpoint, e);
                    }
                }
            }
        }

        Err(PolymarketError::ApiError(
            "All API key endpoints failed — check wallet has MATIC on Polygon".to_string()
        ))
    }

    // ─── Balance ──────────────────────────────────────────────────────────

    pub async fn get_balance(&self) -> Result<f64, PolymarketError> {
        let wallet_addr = format!("{:#042x}", self.wallet.address());

        for (token, name) in &[(PUSD_COLLATERAL, "pUSD"), (USDC_E_TOKEN, "USDC.e"), (USDC_NATIVE, "native USDC")] {
            match get_erc20_balance_onchain(&wallet_addr, token).await {
                Ok(bal) if bal > 0.0 => { tracing::info!("Balance from on-chain {}: {}", name, bal); return Ok(bal); }
                Ok(_) => {}
                Err(e) => { tracing::warn!("On-chain {} query failed: {}", name, e); }
            }
        }

        if self.creds.is_some() {
            let resp = self.build_authed_get("/balance-allowance").send().await;
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    #[derive(Deserialize)]
                    struct BalAllow { #[serde(default)] balance: String, #[serde(default)] allowance: String }
                    if let Ok(body) = resp.json::<BalAllow>().await {
                        if let Ok(balance) = body.balance.parse::<f64>() {
                            if balance > 0.0 { return Ok(balance); }
                        }
                    }
                }
            }
        }

        let addr_lower = self.wallet.address().to_string().to_lowercase();
        if let Ok(resp) = self.http_client.get(format!("{}/value", DATA_HOST))
            .query(&[("user", &addr_lower)]).send().await
        {
            if let Ok(result) = resp.json::<Vec<serde_json::Value>>().await {
                if let Some(val) = result.first().and_then(|v| v.get("value").and_then(|v| v.as_f64())) {
                    return Ok(val);
                }
            }
        }

        Ok(0.0)
    }

    pub async fn get_balance_allowance(&self) -> Result<BalanceAllowance, PolymarketError> {
        let balance = self.get_balance().await?;
        Ok(BalanceAllowance { balance: balance.to_string(), allowance: balance.to_string() })
    }

    pub async fn get_positions(&self) -> Result<Vec<PositionInfo>, PolymarketError> {
        let addr = self.wallet.address().to_string().to_lowercase();
        let resp = self.http_client.get(format!("{}/positions", DATA_HOST))
            .query(&[("user", &addr), ("limit", &"100".to_string())]).send().await?;
        if !resp.status().is_success() {
            return Err(PolymarketError::ApiError(resp.text().await.unwrap_or_default()));
        }
        Ok(resp.json().await?)
    }

    pub async fn validate_credentials(&self) -> Result<ValidationResult, PolymarketError> {
        let balance = self.get_balance().await?;
        Ok(ValidationResult {
            valid: true, wallet_address: self.wallet.address().to_string(), balance,
            message: format!("Credentials valid. Balance: {}", balance),
        })
    }

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

    pub async fn get_market(&self, condition_id: &str) -> Result<MarketResponse, PolymarketError> {
        let resp = self.http_client.get(format!("{}/markets/{}", CLOB_HOST, condition_id)).send().await?;
        if !resp.status().is_success() {
            return Err(PolymarketError::ApiError(resp.text().await.unwrap_or_default()));
        }
        Ok(resp.json().await?)
    }

    // ─── Order Creation (V2) ──────────────────────────────────────────────

    /// Create and sign a V2 order.
    ///
    /// Uses the Python SDK's exact struct:
    ///   Order(uint256 salt,address maker,address signer,uint256 tokenId,
    ///         uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,
    ///         uint256 timestamp,bytes32 metadata,bytes32 builder)
    ///
    /// When `deposit_wallet_address` is provided (or set on the client), uses
    /// POLY_1271 (signatureType=3) with ERC-7739 wrapped signatures.
    pub async fn create_order_v2(
        &self,
        order: &OrderRequest,
        is_neg_risk: bool,
    ) -> Result<serde_json::Value, PolymarketError> {
        let creds = self.creds.as_ref()
            .ok_or(PolymarketError::AuthFailed("No API credentials".to_string()))?;

        let salt_u32: u32 = rand::random::<u32>();
        let salt = U256::from(salt_u32);

        // Timestamp in milliseconds
        let timestamp_ms = U256::from(chrono::Utc::now().timestamp_millis() as u64);
        let empty_metadata = [0u8; 32];
        let empty_builder = [0u8; 32];

        // Amount calculations (6 decimals for USDC)
        // BUY:  makerAmount = USDC spent, takerAmount = shares received
        // SELL: makerAmount = shares sold, takerAmount = USDC received
        let (maker_amount, taker_amount) = if order.side == "BUY" {
            (U256::from((order.size * order.price * 1_000_000.0) as u64),
             U256::from((order.size * 1_000_000.0) as u64))
        } else {
            (U256::from((order.size * 1_000_000.0) as u64),
             U256::from((order.size * order.price * 1_000_000.0) as u64))
        };

        // tokenId (U256)
        let token_id = U256::from_str_radix(&order.token_id, 10).unwrap_or(U256::zero());

        // Determine signature type and maker/signer
        //
        // For POLY_1271 (signature_type=3, deposit wallet flow):
        //   - maker = deposit wallet (funds are there)
        //   - signer = funder address (matches the API key's registered address)
        //   - HMAC POLY_ADDRESS = funder address (matches API key)
        //   - owner = funder address (matches API key)
        //   - The ERC-7739 POLY_1271 signature is signed by the EOA wallet and
        //     verified by the deposit wallet contract
        //
        // For EOA (signature_type=0, standard):
        //   - both maker and signer = EOA wallet address
        let use_poly_1271 = self.signature_type == 3;
        let effective_sig_type: u8 = if use_poly_1271 { 3 } else { self.signature_type };

        let (maker, signer) = if use_poly_1271 {
            // POLY_1271 (signature_type=3, deposit wallet flow):
            //   - maker = deposit wallet contract (where pUSD funds are held)
            //   - signer = deposit wallet (the smart contract verifies the ERC-1271 signature)
            //
            // For POLY_1271, both maker AND signer are the deposit wallet address.
            // The ERC-7739 POLY_1271 signature is signed by the EOA and verified
            // by the deposit wallet contract via isValidSignature().
            //
            // The `owner` field tells the CLOB which API key to use (EOA wallet hex).
            // The POLY_ADDRESS header is the EOA wallet for HMAC auth.
            let deposit_addr = self.deposit_wallet_address
                .unwrap_or_else(|| self.address_h160());
            (deposit_addr, deposit_addr)
        } else {
            // EOA standard: maker = signer = EOA wallet
            let wallet_addr = self.address_h160();
            (wallet_addr, wallet_addr)
        };

        // side: 0 = BUY, 1 = SELL
        let side_u8: u8 = if order.side == "BUY" { 0 } else { 1 };

        // === Compute the contents_hash (order struct hash WITHOUT exchange domain) ===
        let contents_hash = Self::compute_contents_hash(
            &salt, &maker, &signer, &token_id,
            &maker_amount, &taker_amount,
            side_u8, effective_sig_type,
            &timestamp_ms, &empty_metadata, &empty_builder,
        );

        // === Compute the signature ===
        let signature = if use_poly_1271 {
            // Use the CTF Exchange domain separator as app_domain_separator
            let domain_sep = Self::compute_app_domain_separator(Some(&get_exchange_address(is_neg_risk)));

            // Use the actual deposit wallet address as the verifying contract for ERC-7739.
            // Note: `signer` is the funder (matches API key), NOT the deposit wallet.
            let poly_signer = self.deposit_wallet_address
                .unwrap_or_else(|| self.address_h160());
            Self::build_poly_1271_signature(
                &self.private_key_bytes,
                &domain_sep,
                &contents_hash,
                CHAIN_ID,
                &poly_signer,
            )?
        } else {
            // Standard EOA signing
            let domain_sep_array = Self::compute_app_domain_separator(Some(&get_exchange_address(is_neg_risk)));

            // For EOA: struct hash = keccak256(ORDER_TYPE_HASH || abi.encode(order_fields))
            // But wait — the Python SDK for EOA signing uses the FULL EIP-712 digest with
            // the exchange domain, not the raw contents_hash.
            // digest = keccak256(0x19 || 0x01 || exchangeDomainSeparator || orderStructHash)
            // Where orderStructHash = keccak256(ORDER_TYPE_HASH || abi.encode(order_fields))

            // The contents_hash IS the orderStructHash (it includes ORDER_TYPE_HASH prefix)
            let digest = Self::compute_eip712_digest(&domain_sep_array, &contents_hash);
            Self::sign_digest(&self.private_key_bytes, &digest)?
        };

        // === Build JSON payload (matches Python SDK's order_to_json_v2) ===
        // tokenId is passed as the original decimal string (Python SDK passes token_id as-is)
        let side_str = if order.side == "BUY" { "BUY" } else { "SELL" };
        let timestamp_str = timestamp_ms.to_string();
        let maker_hex = format!("{maker:#042x}");
        let signer_hex = format!("{signer:#042x}");

        // The `owner` field tells the CLOB which entity owns this order.
        // For EOA (sig=0): owner = API key string (CLOB maps to EOA wallet)
        // For POLY_1271 (sig=3): owner = EOA wallet address (owner matches signer)
        let owner_str = if use_poly_1271 {
            let eoa_addr = self.address_h160();
            format!("{:#042x}", eoa_addr)
        } else {
            creds.key.clone()
        };

        let signed_order = serde_json::json!({
            "order": {
                "salt": salt_u32,
                "maker": maker_hex,
                "signer": signer_hex,
                "tokenId": &order.token_id,
                "makerAmount": maker_amount.to_string(),
                "takerAmount": taker_amount.to_string(),
                "side": side_str,
                "expiration": "0",
                "signatureType": effective_sig_type,
                "timestamp": timestamp_str,
                "metadata": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "builder": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "signature": signature,
            },
            "owner": owner_str,
            "orderType": "GTC",
            "negRisk": is_neg_risk,
        });

        let json_str = serde_json::to_string_pretty(&signed_order).unwrap_or_default();
        tracing::debug!("Signed order JSON: {}", json_str);

        Ok(signed_order)
    }

    pub async fn create_order(&self, order: &OrderRequest) -> Result<serde_json::Value, PolymarketError> {
        self.create_order_v2(order, false).await
    }

    /// Post order with HMAC signature INCLUDING the request body
    pub async fn post_order(&self, signed_order: &serde_json::Value) -> Result<OrderResponse, PolymarketError> {
        let body_str = serde_json::to_string(signed_order)
            .map_err(|e| PolymarketError::ApiError(format!("JSON serialization error: {e}")))?;

        let response = self.build_authed_post_with_body("/order", &body_str)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }

        let order_response: OrderResponse = response.json().await?;
        Ok(order_response)
    }

    pub async fn cancel_order(&self, order_id: &str) -> Result<(), PolymarketError> {
        let path = format!("/orders/{}", order_id);
        let creds = self.creds.as_ref()
            .ok_or(PolymarketError::AuthFailed("No API credentials".to_string()))?;
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let message = format!("{}DELETE{}", timestamp, path);

        let secret_bytes = Self::decode_api_secret(&creds.secret);

        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).expect("HMAC key size");
        mac.update(message.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

        let url = format!("{}{}", CLOB_HOST, path);
        let response = self.http_client.delete(&url)
            .header("POLY_ADDRESS", self.hmac_address())
            .header("POLY_SIGNATURE", &signature)
            .header("POLY_TIMESTAMP", &timestamp)
            .header("POLY_API_KEY", &creds.key)
            .header("POLY_PASSPHRASE", &creds.passphrase)
            .send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(error_text));
        }
        Ok(())
    }

    pub async fn get_orders(&self) -> Result<Vec<OrderResponse>, PolymarketError> {
        let resp = self.build_authed_get("/orders").send().await?;
        if !resp.status().is_success() {
            return Err(PolymarketError::ApiError(resp.text().await.unwrap_or_default()));
        }
        Ok(resp.json().await?)
    }

    // ─── Deposit Wallet Operations ────────────────────────────────────────

    /// Check if a deposit wallet exists at the given address.
    /// Uses eth_getCode to check for deployed contract code.
    pub async fn check_deposit_wallet_exists(address: &str) -> Result<bool, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getCode",
            "params": [address, "latest"],
            "id": 1,
        });
        let (_rpc_url, result) = try_polygon_rpcs(&body).await?;
        let code = result.get("result").and_then(|v| v.as_str()).unwrap_or("0x");
        Ok(code.len() > 2) // "0x" means no code
    }

    /// Query the deposit wallet address from the Polymarket relayer API.
    /// Uses Gamma auth (same CLOB credentials) to call GET /relayer/api/keys.
    /// Returns the deposit wallet address if found, or None if no keys exist.
    pub async fn query_deposit_wallet_address_from_relayer(&self) -> Result<Option<String>, PolymarketError> {
        #[derive(Deserialize)]
        struct RelayerKey {
            #[serde(default)]
            address: String,
            #[serde(default)]
            api_key: String,
        }

        let resp = self.build_relayer_get("/relayer/api/keys")
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PolymarketError::ApiError(format!(
                "Relayer API responded with {}: {}", status, text
            )));
        }

        let keys: Vec<RelayerKey> = resp.json().await.unwrap_or_default();

        // The relayer keys response contains the deposit wallet address in the `address` field.
        // The first key's address is our deposit wallet.
        if let Some(key) = keys.first() {
            if !key.address.is_empty() && key.address != "0x" {
                tracing::info!("Found relayer key {} with deposit wallet address: {}", key.api_key, key.address);
                return Ok(Some(key.address.clone()));
            }
        }

        Ok(None)
    }
}

// ─── Free Functions ────────────────────────────────────────────────────────

pub async fn try_polygon_rpcs(body: &serde_json::Value) -> Result<(String, serde_json::Value), String> {
    let client = reqwest::Client::new();
    for rpc_url in POLYGON_RPCS {
        match client.post(*rpc_url).timeout(Duration::from_secs(5)).json(body).send().await {
            Ok(resp) => {
                match resp.json::<serde_json::Value>().await {
                    Ok(result) => {
                        if let Some(error) = result.get("error") {
                            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                            tracing::warn!("Polygon RPC {} error: {}", rpc_url, msg);
                            continue;
                        }
                        return Ok((rpc_url.to_string(), result));
                    }
                    Err(e) => { tracing::warn!("Polygon RPC {} parse error: {}", rpc_url, e); continue; }
                }
            }
            Err(e) => { tracing::warn!("Polygon RPC {} error: {}", rpc_url, e); continue; }
        }
    }
    Err("All Polygon RPC endpoints failed".to_string())
}

pub async fn check_matic_balance(wallet_address: &str) -> Result<f64, String> {
    let body = serde_json::json!({"jsonrpc":"2.0","method":"eth_getBalance","params":[wallet_address,"latest"],"id":1});
    let (_rpc_url, result) = try_polygon_rpcs(&body).await?;
    let balance_hex = result.get("result").and_then(|v| v.as_str()).ok_or("Invalid balance response")?;
    let balance_wei = u128::from_str_radix(balance_hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Parse error: {e}"))?;
    Ok(balance_wei as f64 / 1e18)
}

pub async fn get_erc20_balance_onchain(wallet_address: &str, token_address: &str) -> Result<f64, String> {
    let selector = "70a08231";
    let addr = wallet_address.trim_start_matches("0x").to_lowercase();
    let data = format!("0x{}{:0>64}", selector, addr);
    let body = serde_json::json!({"jsonrpc":"2.0","method":"eth_call","params":[{"to":token_address,"data":data},"latest"],"id":1});
    let (_rpc_url, result) = try_polygon_rpcs(&body).await?;
    let balance_hex = result.get("result").and_then(|v| v.as_str()).ok_or("Invalid response")?;
    if balance_hex == "0x" || balance_hex == "0x0" { return Ok(0.0); }
    let balance_raw = u128::from_str_radix(balance_hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Parse error: {e}"))?;
    Ok(balance_raw as f64 / 1_000_000.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
    pub funder: Option<String>,
    pub signature_type: u8,
    pub wallet_address: String,
}

/// Standalone helper: derive API key credentials for a given private key's wallet address.
/// Creates a temporary client, calls create_or_derive_api_key, and returns the credentials.
/// This is useful for ensuring the API key is registered to the correct address before placing orders.
pub async fn derive_api_key_for_private_key(private_key: &str) -> Result<ApiKeyCreds, PolymarketError> {
    let mut client = PolymarketClient::new(private_key)?;
    client.create_or_derive_api_key().await
}

/// Force-create a NEW API key registered to the EOA wallet.
///
/// Unlike `derive_api_key_for_private_key`, this function SKIPS the
/// `GET /auth/derive-api-key` step (which returns an existing key that
/// might be registered to an unknown address). Instead, it only attempts
/// to CREATE a new key via `POST /auth/api-key` with a random nonce.
///
/// This guarantees the returned key is registered to the EOA wallet
/// (the address derived from the private key).
pub async fn create_new_api_key_for_eoa(private_key: &str) -> Result<ApiKeyCreds, PolymarketError> {
    let client = PolymarketClient::new(private_key)?;
    let address = client.address_h160();
    let address_str = format!("{:#042x}", address);
    let message_str = "This message attests that I control the given wallet";

    // Attempt 1: POST /auth/api-key with random nonce (create new key)
    {
        let ts = chrono::Utc::now().timestamp().to_string();
        let random_nonce = U256::from(rand::random::<u32>());
        let sig = PolymarketClient::compute_clob_auth_signature(
            &client.private_key_bytes, address, &ts, random_nonce, message_str,
        )?;

        let body = serde_json::json!({});
        let url = format!("{}/auth/api-key", CLOB_HOST);
        let resp = client.http_client
            .post(&url)
            .header("POLY_ADDRESS", &address_str)
            .header("POLY_SIGNATURE", &sig)
            .header("POLY_TIMESTAMP", &ts)
            .header("POLY_NONCE", random_nonce.to_string())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(raw) = r.json::<serde_json::Value>().await {
                    if let Ok(creds) = PolymarketClient::parse_api_key_response(&raw) {
                        tracing::info!("Created NEW API key for EOA wallet (POST /auth/api-key, random nonce)");
                        return Ok(creds);
                    }
                }
            }
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                tracing::warn!("POST /auth/api-key (random nonce) failed: {}: {}", status, text);
            }
            Err(e) => {
                tracing::warn!("POST /auth/api-key (random nonce) error: {}", e);
            }
        }
    }

    // Attempt 2 (fallback): old endpoints with old authentication
    {
        let old_timestamp = chrono::Utc::now().timestamp_millis();
        let old_message = format!("Sign this message to authenticate with Polymarket.\n\nTimestamp: {}", old_timestamp);
        let old_signature = client.wallet.sign_message(old_message.as_bytes()).await
            .map_err(|e| PolymarketError::SignatureFailed(e.to_string()))?;
        let old_body = serde_json::json!({
            "address": client.address(),
            "message": old_message,
            "signature": old_signature.to_string()
        });

        for endpoint in &["/api/keys", "/api-keys", "/api_key"] {
            let response = client.http_client.post(format!("{}{}", CLOB_HOST, endpoint))
                .header("Content-Type", "application/json")
                .json(&old_body)
                .send()
                .await;
            match response {
                Ok(r) if r.status().is_success() => {
                    if let Ok(raw) = r.json::<serde_json::Value>().await {
                        if let Ok(creds) = PolymarketClient::parse_api_key_response(&raw) {
                            tracing::info!("Created NEW API key for EOA wallet via fallback endpoint {}", endpoint);
                            return Ok(creds);
                        }
                    }
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    tracing::warn!("Fallback {} failed: {}: {}", endpoint, status, text);
                }
                Err(e) => {
                    tracing::warn!("Fallback {} error: {}", endpoint, e);
                }
            }
        }
    }

    Err(PolymarketError::ApiError(
        "All new API key creation endpoints failed — check wallet has MATIC on Polygon".to_string()
    ))
}

/// Standalone helper: derive API key credentials for a specific target address
/// (e.g., a deposit wallet address) using the EOA private key for ClobAuth signing.
/// The EOA signs a ClobAuth message claiming the target address, allowing the Polymarket
/// API to create an API key registered to the target address.
pub async fn derive_api_key_for_target_address(
    private_key: &str,
    target_address: &str,
) -> Result<ApiKeyCreds, PolymarketError> {
    let key_bytes = hex::decode(private_key.trim_start_matches("0x"))
        .map_err(|e| PolymarketError::InvalidKey(e.to_string()))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| PolymarketError::RequestFailed(e))?;

    let address: H160 = target_address.parse()
        .map_err(|e| PolymarketError::InvalidKey(format!("Invalid target address: {e}")))?;
    let address_str = format!("{:#042x}", address);
    let ts = chrono::Utc::now().timestamp().to_string();
    let nonce = U256::zero();
    let message_str = "This message attests that I control the given wallet";

    let sig = PolymarketClient::compute_clob_auth_signature(
        &key_bytes, address, &ts, nonce, message_str,
    )?;

    let body = serde_json::json!({});
    let url = format!("{}/auth/api-key", CLOB_HOST);
    let resp = client
        .post(&url)
        .header("POLY_ADDRESS", &address_str)
        .header("POLY_SIGNATURE", &sig)
        .header("POLY_TIMESTAMP", &ts)
        .header("POLY_NONCE", "0")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    if resp.as_ref().map(|r| r.status().is_success()).unwrap_or(false) {
        if let Ok(raw) = resp.unwrap().json::<serde_json::Value>().await {
            if let Ok(creds) = PolymarketClient::parse_api_key_response(&raw) {
                tracing::info!("Created API key for address {} (via EOA ClobAuth)", target_address);
                return Ok(creds);
            }
        }
        return Err(PolymarketError::ApiError("API key response parse failed".to_string()));
    }

    // Fallback: try GET /auth/derive-api-key (key may already exist)
    tracing::info!("POST /auth/api-key failed for {}, trying GET /auth/derive-api-key...", target_address);
    {
        let ts = chrono::Utc::now().timestamp().to_string();
        let nonce = U256::zero();
        let sig = match PolymarketClient::compute_clob_auth_signature(
            &key_bytes, address, &ts, nonce, message_str,
        ) {
            Ok(s) => s,
            Err(e) => return Err(PolymarketError::SignatureFailed(e.to_string())),
        };

        let url = format!("{}/auth/derive-api-key", CLOB_HOST);
        let resp = client
            .get(&url)
            .header("POLY_ADDRESS", &address_str)
            .header("POLY_SIGNATURE", &sig)
            .header("POLY_TIMESTAMP", &ts)
            .header("POLY_NONCE", "0")
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(raw) = r.json::<serde_json::Value>().await {
                    if let Ok(creds) = PolymarketClient::parse_api_key_response(&raw) {
                        tracing::info!("Derived existing API key for address {} via GET /auth/derive-api-key", target_address);
                        return Ok(creds);
                    }
                }
                return Err(PolymarketError::ApiError("Derive API key response parse failed".to_string()));
            }
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                tracing::warn!("GET /auth/derive-api-key for target {} failed: {}: {}", target_address, status, text);
            }
            Err(e) => {
                tracing::warn!("GET /auth/derive-api-key for target {} error: {}", target_address, e);
            }
        }
    }

    // Third attempt: POST /auth/api-key with random nonce
    tracing::info!("GET /auth/derive-api-key also failed for {}, trying POST /auth/api-key with random nonce...", target_address);
    {
        let ts = chrono::Utc::now().timestamp().to_string();
        let random_nonce = U256::from(rand::random::<u32>());
        let sig = PolymarketClient::compute_clob_auth_signature(
            &key_bytes, address, &ts, random_nonce, message_str,
        )?;

        let body = serde_json::json!({});
        let url = format!("{}/auth/api-key", CLOB_HOST);
        let resp = client
            .post(&url)
            .header("POLY_ADDRESS", &address_str)
            .header("POLY_SIGNATURE", &sig)
            .header("POLY_TIMESTAMP", &ts)
            .header("POLY_NONCE", random_nonce.to_string())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(raw) = r.json::<serde_json::Value>().await {
                    if let Ok(creds) = PolymarketClient::parse_api_key_response(&raw) {
                        tracing::info!("Created API key for address {} via POST /auth/api-key (random nonce)", target_address);
                        return Ok(creds);
                    }
                }
                return Err(PolymarketError::ApiError("API key random-nonce response parse failed".to_string()));
            }
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                return Err(PolymarketError::ApiError(format!(
                    "API key for {} failed all 3 attempts (POST nonce=0, GET derive, POST random nonce). Last: {}: {}",
                    target_address, status, text
                )));
            }
            Err(e) => {
                return Err(PolymarketError::RequestFailed(e));
            }
        }
    }
}

/// Try to auto-detect the deposit wallet address for a user from the Polymarket relayer API.
///
/// This is needed when the user's Polymarket account uses a profile/funder address
/// that differs from the EOA signer wallet. Such accounts require POLY_1271 (signature_type=3)
/// order signing via a deposit wallet contract.
///
/// Returns the deposit wallet address if found, or None if the relayer has no keys.
pub async fn try_fetch_deposit_wallet(
    private_key: &str,
    api_key: &str,
    api_secret: &str,
    api_passphrase: &str,
    funder: Option<&str>,
) -> Result<Option<String>, PolymarketError> {
    let client = PolymarketClient::from_api_credentials(
        private_key,
        3, // Use POLY_1271 sig type for proper relayer auth headers
        Some(ApiKeyCreds {
            key: api_key.to_string(),
            secret: api_secret.to_string(),
            passphrase: api_passphrase.to_string(),
        }),
        funder,
        None,
    )?;

    client.query_deposit_wallet_address_from_relayer().await
}

/// Encode a call to the Deposit Wallet Factory's `deploy(address[],bytes32[])` function.
/// Returns the ABI-encoded calldata.
pub fn encode_deploy_call(user_address: H160, salt: [u8; 32]) -> Vec<u8> {
    use ethers::abi::Token;
    let selector = compute_selector_raw(b"deploy(address[],bytes32[])");
    let owners_token = Token::Array(vec![Token::Address(user_address)]);
    let ids_token = Token::Array(vec![Token::FixedBytes(salt.to_vec())]);
    let encoded = ethers::abi::encode(&[owners_token, ids_token]);
    let mut data = Vec::with_capacity(4 + encoded.len());
    data.extend_from_slice(&selector);
    data.extend(encoded);
    data
}

/// Compute keccak256 function selector (first 4 bytes) from a function signature string.
fn compute_selector_raw(sig: &[u8]) -> [u8; 4] {
    let hash = ethers::utils::keccak256(sig);
    [hash[0], hash[1], hash[2], hash[3]]
}

pub fn get_exchange_address(is_neg_risk: bool) -> String {
    if is_neg_risk { NEG_RISK_CTF_EXCHANGE_V2.to_string() } else { CTF_EXCHANGE_V2.to_string() }
}

pub fn get_collateral_address() -> String { PUSD_COLLATERAL.to_string() }
pub fn get_collateral_onramp() -> String { COLLATERAL_ONRAMP.to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let pk = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let client = PolymarketClient::new(pk).unwrap();
        assert!(client.address().starts_with("0x"));
    }

    #[test]
    fn test_invalid_key() { assert!(PolymarketClient::new("invalid").is_err()); }

    #[test]
    fn test_contents_hash_matches_sdk_format() {
        // Verify the ORDER_TYPE_STRING matches Python SDK exactly
        let expected = "Order(uint256 salt,address maker,address signer,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)";
        assert_eq!(ORDER_TYPE_STRING, expected, "ORDER_TYPE_STRING must match Python SDK exactly");
    }

    #[test]
    fn test_solady_type_string_matches_sdk() {
        let expected_prefix = "TypedDataSign(Order contents,string name,string version,uint256 chainId,address verifyingContract,bytes32 salt)";
        assert!(
            SOLADY_TYPE_STRING.starts_with(expected_prefix),
            "SOLADY_TYPE_STRING must start with the expected prefix"
        );
        assert!(
            SOLADY_TYPE_STRING.contains("Order("),
            "SOLADY_TYPE_STRING must contain Order type"
        );
    }

    #[test]
    fn test_compute_app_domain_separator() {
        // Just verify it doesn't panic and returns 32 bytes
        let sep = PolymarketClient::compute_app_domain_separator(None);
        assert_eq!(sep.len(), 32);
    }

    #[test]
    fn test_compute_contents_hash() {
        // Just verify the function works
        let salt = U256::from(12345u64);
        let maker: H160 = "0x6cd13c8e4e9b77be31cae38080b488b59d569227".parse().unwrap();
        let signer = maker;
        let token_id = U256::from(123456789u64);
        let maker_amount = U256::from(200000u64);
        let taker_amount = U256::from(400833u64);
        let metadata = [0u8; 32];
        let builder = [0u8; 32];
        let timestamp = U256::from(1780605349626u64);

        let hash = PolymarketClient::compute_contents_hash(
            &salt, &maker, &signer, &token_id,
            &maker_amount, &taker_amount,
            0, 0, &timestamp, &metadata, &builder,
        );
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_compute_solady_struct_hash() {
        let contents_hash = [0u8; 32];
        let addr: H160 = "0x6cd13c8e4e9b77be31cae38080b488b59d569227".parse().unwrap();
        let hash = PolymarketClient::compute_solady_struct_hash(
            &contents_hash, 137, &addr,
        );
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_sign_digest() {
        let pk_bytes = hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();
        let digest = [0x01u8; 32];
        let sig = PolymarketClient::sign_digest(&pk_bytes, &digest).unwrap();
        assert!(sig.starts_with("0x"));
        // signature should be 130 hex chars (65 bytes) for ECDSA
        assert_eq!(sig.len(), 132, "ECDSA signature should be 65 bytes = 130 hex chars + 0x");
    }

    #[test]
    fn test_build_poly_1271_signature() {
        let pk_bytes = hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();
        let app_domain_sep = [0x02u8; 32];
        let contents_hash = [0x03u8; 32];
        let addr: H160 = "0x6cd13c8e4e9b77be31cae38080b488b59d569227".parse().unwrap();

        let sig = PolymarketClient::build_poly_1271_signature(
            &pk_bytes, &app_domain_sep, &contents_hash, 137, &addr,
        ).unwrap();
        assert!(sig.starts_with("0x"));
        // POLY_1271 signature is long: 0x + 130 (inner_sig) + 64 (domain_sep) + 64 (contents_hash) + hex(ORDER_TYPE_STRING) + 4(type_len)
        assert!(sig.len() > 300, "POLY_1271 signature should be >300 hex chars, got {}", sig.len());
    }
}
