use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Ticker {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub price_change: String,
    #[serde(rename = "P")]
    pub price_change_percent: String,
    #[serde(rename = "w")]
    pub weighted_avg_price: String,
    #[serde(rename = "c")]
    pub last_price: String,
    #[serde(rename = "Q")]
    pub last_qty: String,
    #[serde(rename = "o")]
    pub open_price: String,
    #[serde(rename = "h")]
    pub high_price: String,
    #[serde(rename = "l")]
    pub low_price: String,
    #[serde(rename = "v")]
    pub total_volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "O")]
    pub stats_open_time: i64,
    #[serde(rename = "C")]
    pub stats_close_time: i64,
    #[serde(rename = "F")]
    pub first_trade_id: i64,
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    #[serde(rename = "n")]
    pub num_trades: i64,
}

impl Ticker {
    pub fn last_price_f64(&self) -> f64 {
        self.last_price.parse().unwrap_or(0.0)
    }

    pub fn price_change_percent_f64(&self) -> f64 {
        self.price_change_percent.parse().unwrap_or(0.0)
    }

    pub fn high_price_f64(&self) -> f64 {
        self.high_price.parse().unwrap_or(0.0)
    }

    pub fn low_price_f64(&self) -> f64 {
        self.low_price.parse().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceUpdate {
    pub price: f64,
    pub timestamp: i64,
    pub price_change: f64,
    pub price_change_percent: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
}

impl From<&Ticker> for PriceUpdate {
    fn from(ticker: &Ticker) -> Self {
        Self {
            price: ticker.last_price_f64(),
            timestamp: ticker.event_time,
            price_change: ticker.price_change.parse().unwrap_or(0.0),
            price_change_percent: ticker.price_change_percent_f64(),
            high_24h: ticker.high_price_f64(),
            low_24h: ticker.low_price_f64(),
            volume_24h: ticker.quote_volume.parse().unwrap_or(0.0),
        }
    }
}
