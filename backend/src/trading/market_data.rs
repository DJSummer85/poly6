//! Market Data Service - Fetches and aggregates market data for strategy evaluation
//! Provides MarketSnapshot with full Polymarket + Binance context

use crate::trading::bot_executor::strategies::MarketSnapshot;
use std::time::{SystemTime, UNIX_EPOCH};

const GAMMA_API: &str = "https://gamma-api.polymarket.com";

#[derive(Debug, Clone)]
pub struct MarketDataService {
    http_client: reqwest::Client,
}

impl MarketDataService {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    /// Fetch complete market snapshot for a given market_id (condition_id)
    pub async fn get_snapshot(&self, market_id: &str) -> Result<MarketSnapshot, String> {
        let mut snapshot = MarketSnapshot::new(market_id.to_string());
        snapshot.fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        // Fetch Polymarket market data from gamma API
        match self.fetch_polymarket_data(market_id).await {
            Ok(data) => {
                snapshot.yes_price = data.0;
                snapshot.no_price = data.1;
                snapshot.spread = (snapshot.no_price - snapshot.yes_price).abs();
                snapshot.volume = data.2;
                snapshot.liquidity = data.3;
                snapshot.time_remaining = data.4;
                snapshot.question = data.5;
            }
            Err(e) => {
                tracing::warn!("Failed to fetch Polymarket data for {}: {}", market_id, e);
            }
        }

        // Fetch BTC data from Binance
        match self.fetch_btc_data().await {
            Ok(data) => {
                snapshot.btc_price = data.0;
                snapshot.btc_change_24h = data.1;
                snapshot.btc_velocity = data.2;
                snapshot.btc_acceleration = data.3;
                snapshot.btc_volatility = data.4;
                snapshot.btc_window_open = data.5;
            }
            Err(e) => {
                tracing::warn!("Failed to fetch BTC data: {}", e);
            }
        }

        Ok(snapshot)
    }

    async fn fetch_polymarket_data(
        &self,
        market_id: &str,
    ) -> Result<(f64, f64, f64, f64, i64, String), String> {
        let response = self
            .http_client
            .get(format!("{}/markets/{}", GAMMA_API, market_id))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("Gamma API request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Gamma API error: {}", response.status()));
        }

        #[derive(serde::Deserialize)]
        struct GammaMarket {
            question: String,
            #[serde(default)]
            outcome_prices: Option<String>,
            #[serde(rename = "volume_num", default)]
            volume: Option<f64>,
            #[serde(default)]
            liquidity: Option<f64>,
            #[serde(rename = "endDate", default)]
            end_date: Option<String>,
        }

        let market: GammaMarket = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse gamma response: {}", e))?;

        let (yes_price, no_price) = if let Some(ref prices) = market.outcome_prices {
            let prices: Vec<f64> = serde_json::from_str(prices).map_err(|e| format!("Bad prices JSON: {}", e))?;
            match (prices.first(), prices.get(1)) {
                (Some(&yes), Some(&no)) => (yes, no),
                (Some(&yes), None) => (yes, 1.0 - yes),
                _ => (0.5, 0.5),
            }
        } else {
            (0.5, 0.5)
        };

        let volume = market.volume.unwrap_or(0.0);
        let liquidity = market.liquidity.unwrap_or(0.0);
        let question = market.question;

        // Parse end_date (ISO8601) to time_remaining
        let time_remaining = if let Some(ref date_str) = market.end_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
                let expire_ts = dt.timestamp();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                (expire_ts - now).max(0)
            } else {
                0
            }
        } else {
            0
        };

        Ok((yes_price, no_price, volume, liquidity, time_remaining, question))
    }

    async fn fetch_btc_data(&self) -> Result<(f64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>), String> {
        // Get current BTC price
        let response = self
            .http_client
            .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
            .send()
            .await
            .map_err(|e| format!("Binance request failed: {}", e))?;

        if !response.status().is_success() {
            return Err("Binance API error".to_string());
        }

        #[derive(serde::Deserialize)]
        struct BinancePrice {
            price: String,
        }

        let data: BinancePrice = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Binance response: {}", e))?;

        let btc_price: f64 = data.price.parse().map_err(|e| format!("BTC price parse error: {}", e))?;

        // Get 24h ticker for change
        let ticker_response = self
            .http_client
            .get("https://api.binance.com/api/v3/ticker/24hr?symbol=BTCUSDT")
            .send()
            .await
            .map_err(|e| format!("Binance ticker request failed: {}", e))?;

        let btc_change_24h = if ticker_response.status().is_success() {
            #[derive(serde::Deserialize)]
            struct BinanceTicker {
                #[serde(rename = "priceChangePercent")]
                price_change_percent: Option<String>,
            }
            let ticker: BinanceTicker = ticker_response
                .json()
                .await
                .map_err(|e| format!("Failed to parse ticker: {}", e))?;
            ticker.price_change_percent
                .and_then(|s| s.parse::<f64>().ok())
                .map(|p| p / 100.0)
        } else {
            None
        };

        Ok((btc_price, btc_change_24h, None, None, None, None))
    }
}

impl Default for MarketDataService {
    fn default() -> Self {
        Self::new()
    }
}
