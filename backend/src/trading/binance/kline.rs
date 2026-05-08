use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct KlineData {
    #[serde(rename = "t")]
    pub kline_start_time: i64,
    #[serde(rename = "T")]
    pub kline_close_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    #[serde(rename = "o")]
    pub open_price: String,
    #[serde(rename = "c")]
    pub close_price: String,
    #[serde(rename = "h")]
    pub high_price: String,
    #[serde(rename = "l")]
    pub low_price: String,
    #[serde(rename = "v")]
    pub base_volume: String,
    #[serde(rename = "n")]
    pub num_trades: i64,
    #[serde(rename = "x")]
    pub is_closed: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_base_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KlineMessage {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: KlineData,
}

impl KlineData {
    pub fn open_price_f64(&self) -> f64 {
        self.open_price.parse().unwrap_or(0.0)
    }

    pub fn close_price_f64(&self) -> f64 {
        self.close_price.parse().unwrap_or(0.0)
    }

    pub fn high_price_f64(&self) -> f64 {
        self.high_price.parse().unwrap_or(0.0)
    }

    pub fn low_price_f64(&self) -> f64 {
        self.low_price.parse().unwrap_or(0.0)
    }

    pub fn volume_f64(&self) -> f64 {
        self.base_volume.parse().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KlineUpdate {
    pub start_time: i64,
    pub end_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub is_closed: bool,
    pub interval: String,
}

impl From<&KlineData> for KlineUpdate {
    fn from(k: &KlineData) -> Self {
        Self {
            start_time: k.kline_start_time,
            end_time: k.kline_close_time,
            open: k.open_price_f64(),
            high: k.high_price_f64(),
            low: k.low_price_f64(),
            close: k.close_price_f64(),
            volume: k.volume_f64(),
            is_closed: k.is_closed,
            interval: k.interval.clone(),
        }
    }
}
