use axum::{
    extract::Query,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use serde::Deserialize;

const GAMMA_API: &str = "https://gamma-api.polymarket.com";
const CLOB_API: &str = "https://clob.polymarket.com";

// Map timeframe to duration in seconds
pub const TIMEFRAME_DURATIONS: [(&str, u64); 5] = [
    ("5", 300),      // 5 minutes
    ("15", 900),     // 15 minutes
    ("60", 3600),    // 1 hour
    ("240", 14400),  // 4 hours
    ("D", 86400),    // 1 day
];

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MarketPriceRequest {
    pub token_id: Option<String>,
    pub condition_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MarketPriceResponse {
    pub success: bool,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub token_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BitcoinPriceResponse {
    pub success: bool,
    pub price: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveMarket {
    pub condition_id: String,
    pub question: String,
    pub description: Option<String>,
    pub yes_token_id: String,
    pub no_token_id: String,
    pub yes_price: f64,
    pub no_price: f64,
    pub start_time: i64,    // Unix timestamp
    pub end_time: i64,      // Unix timestamp
    pub time_remaining: i64, // Seconds until end
    pub volume: f64,
    pub liquidity: f64,
    pub asset: String,      // BTC, ETH, SOL, XRP
    pub timeframe: String,  // 5, 15, 60, 240, D
    pub price_to_beat: Option<f64>, // Settlement price
    pub status: String,     // "active", "closed", "settled"
    pub category: String,   // "BTC 5", "ETH 15", etc.
}

#[derive(Debug, Serialize)]
pub struct ActiveMarketsResponse {
    pub success: bool,
    pub markets: Vec<ActiveMarket>,
    pub count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MarketDetail {
    pub condition_id: String,
    pub question: String,
    pub description: Option<String>,
    pub yes_token_id: Option<String>,
    pub no_token_id: Option<String>,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub volume: Option<f64>,
    pub liquidity: Option<f64>,
    pub end_date: Option<String>,
    pub price_to_beat: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct MarketListResponse {
    pub success: bool,
    pub markets: Vec<MarketDetail>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActiveMarketsQuery {
    pub timeframe: Option<String>,
    pub asset: Option<String>,
}

/// Parse clobTokenIds string into tokens array format.
pub fn parse_clob_token_ids(clob_token_ids: &str) -> Option<Vec<(String, String)>> {
    let ids: Vec<String> = serde_json::from_str(clob_token_ids).ok()?;
    if ids.len() < 2 {
        return None;
    }
    Some(vec![
        (ids[0].clone(), "Up".to_string()),
        (ids[1].clone(), "Down".to_string()),
    ])
}

/// Get duration for timeframe
pub fn get_duration_for_timeframe(timeframe: &str) -> u64 {
    TIMEFRAME_DURATIONS
        .iter()
        .find(|(tf, _)| tf == &timeframe)
        .map(|(_, dur)| *dur)
        .unwrap_or(300)
}

/// Get timeframe slug suffix (5m, 15m, 1h, 4h, 1d)
fn get_timeframe_slug(timeframe: &str) -> &'static str {
    match timeframe {
        "D" => "1d",
        "240" => "4h",
        "60" => "1h",
        "15" => "15m",
        _ => "5m",
    }
}

/// Custom deserializer that accepts both string and number for f64 fields
/// Used for fields like liquidity which can be returned as string or number
fn deserialize_f64_from_string_or_number<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct F64Visitor;

    impl<'de> Visitor<'de> for F64Visitor {
        type Value = Option<f64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a f64 number, a string containing a number, or null")
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v as f64))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v as f64))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.parse::<f64>()
                .map(Some)
                .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D2>(self, deserializer2: D2) -> Result<Self::Value, D2::Error>
        where
            D2: serde::Deserializer<'de>,
        {
            deserializer2.deserialize_any(self)
        }
    }

    deserializer.deserialize_any(F64Visitor)
}

/// Fetch a single market by slug from Gamma API
pub async fn fetch_market_by_slug(slug: &str, asset: &str, timeframe: &str) -> Option<ActiveMarket> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/events/slug/{slug}", GAMMA_API))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            #[derive(Deserialize)]
            struct PolymarketEvent {
                active: Option<bool>,
                closed: Option<bool>,
                #[serde(default)]
                description: Option<String>,
                markets: Option<Vec<PolymarketMarket>>,
                resolution_source: Option<String>,
                #[serde(default)]
                image: Option<String>,
                #[serde(default, deserialize_with = "deserialize_f64_from_string_or_number")]
                volume: Option<f64>,
                #[serde(default, deserialize_with = "deserialize_f64_from_string_or_number")]
                liquidity_clob: Option<f64>,
                #[serde(default)]
                event_metadata: Option<EventMetadata>,
            }

            #[derive(Deserialize, Default)]
            struct EventMetadata {
                #[serde(default)]
                price_to_beat: Option<f64>,
            }

            #[derive(Deserialize)]
            struct PolymarketMarket {
                #[serde(default)]
                id: Option<String>,
                #[serde(rename = "conditionId")]
                condition_id: String,
                question: String,
                #[serde(default)]
                description: Option<String>,
                #[serde(default)]
                active: Option<bool>,
                #[serde(default)]
                closed: Option<bool>,
                #[serde(rename = "endDate")]
                end_date: Option<String>,
                #[serde(rename = "outcomePrices", default)]
                outcome_prices: Option<String>,
                #[serde(rename = "volumeNum", default, deserialize_with = "deserialize_f64_from_string_or_number")]
                volume_num: Option<f64>,
                #[serde(default, deserialize_with = "deserialize_f64_from_string_or_number")]
                liquidity: Option<f64>,
                #[serde(rename = "clobTokenIds", default)]
                clob_token_ids: Option<String>,
                #[serde(default)]
                tokens: Option<Vec<MarketToken>>,
            }

            #[derive(Deserialize)]
            struct MarketToken {
                #[serde(rename = "tokenId", default)]
                token_id: Option<String>,
                #[serde(default)]
                outcome: Option<String>,
            }

            // Parse response, return None if parsing fails
            let event: PolymarketEvent = resp.json().await.ok()?;

            // Take ownership of markets and get first one
            // Note: Event-level closed status can be true even when market inside is active
            // We check market-level status instead below
            let markets = event.markets.unwrap_or_default();
            if markets.is_empty() {
                return None;
            }
            let market = &markets[0];

            // Check if market is active (not closed)
            if market.closed == Some(true) {
                return None;
            }

            // Parse end time and check if market hasn't expired
            let end_time = market.end_date
                .as_ref()
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .map(|dt| dt.timestamp())?;

            if end_time < chrono::Utc::now().timestamp() {
                return None;
            }

            // Parse outcome prices
            let outcome_prices: Vec<String> = market.outcome_prices
                .as_ref()
                .and_then(|p| serde_json::from_str(p).ok())
                .unwrap_or_else(|| vec!["0.5".to_string(), "0.5".to_string()]);

            let yes_price = outcome_prices.first()
                .and_then(|p| p.parse::<f64>().ok())
                .unwrap_or(0.5);
            let no_price = 1.0 - yes_price;

            // Calculate start time from end time and duration
            let duration_secs = get_duration_for_timeframe(timeframe);
            let start_time = end_time - duration_secs as i64;
            let time_remaining = end_time - chrono::Utc::now().timestamp();

            // Get token IDs
            let tokens = parse_clob_token_ids(market.clob_token_ids.as_deref().unwrap_or(""))
                .or_else(|| {
                    market.tokens.as_ref().map(|t| {
                        t.iter()
                            .filter_map(|tok| {
                                tok.token_id.as_ref().zip(tok.outcome.as_ref())
                                    .map(|(id, out)| (id.clone(), out.clone()))
                            })
                            .collect::<Vec<_>>()
                    })
                });

            let (yes_token_id, no_token_id) = tokens
                .map(|t| {
                    let up_token = t.iter()
                        .find(|(_, out)| out == "Up" || out == "Long" || out.to_lowercase().contains("up"))
                        .map(|(id, _)| id.clone())
                        .unwrap_or_else(|| t.first().map(|(id, _)| id.clone()).unwrap_or_default());
                    let down_token = t.iter()
                        .find(|(_, out)| out == "Down" || out == "Short" || out.to_lowercase().contains("down"))
                        .map(|(id, _)| id.clone())
                        .unwrap_or_else(|| t.get(1).map(|(id, _)| id.clone()).unwrap_or_default());
                    (up_token, down_token)
                })
                .unwrap_or_default();

            let volume = market.volume_num.unwrap_or(event.volume.unwrap_or(0.0));
            let liquidity = market.liquidity.unwrap_or(event.liquidity_clob.unwrap_or(0.0));
            let price_to_beat = event.event_metadata.as_ref()
                .and_then(|m| m.price_to_beat);

            Some(ActiveMarket {
                condition_id: market.condition_id.clone(),
                question: market.question.clone(),
                description: market.description.clone().or(event.description),
                yes_token_id,
                no_token_id,
                yes_price,
                no_price,
                start_time,
                end_time,
                time_remaining,
                volume,
                liquidity,
                asset: asset.to_uppercase(),
                timeframe: timeframe.to_string(),
                price_to_beat,
                status: "active".to_string(),
                category: format!("{} {}", asset.to_uppercase(), timeframe),
            })
        }
        _ => None,
    }
}

/// Fetch active BTC/ETH/SOL/XRP up/down markets for a timeframe
pub async fn fetch_active_markets(timeframe: &str) -> Vec<ActiveMarket> {
    let duration = get_duration_for_timeframe(timeframe);
    let now_ts = chrono::Utc::now().timestamp();
    let rounded_time = (now_ts / duration as i64) * duration as i64;

    let assets = ["btc", "eth", "sol", "xrp"];
    let tf_slug = get_timeframe_slug(timeframe);

    let mut markets: Vec<ActiveMarket> = Vec::new();

    // Parallel fetch for all assets and offsets
    for asset in assets {
        for offset in 0..4 {
            let try_time = rounded_time - offset * duration as i64;
            let slug = format!("{asset}-updown-{tf_slug}-{try_time}");

            if let Some(market) = fetch_market_by_slug(&slug, asset, timeframe).await {
                // Only add if not already present (unique condition_id)
                if !markets.iter().any(|m| m.condition_id == market.condition_id) {
                    markets.push(market);
                    break; // Found active market for this asset, stop checking offsets
                }
            }
        }
    }

    // Sort by time remaining (soonest first)
    markets.sort_by_key(|m| m.time_remaining);

    tracing::info!("Found {} active markets for {} timeframe", markets.len(), timeframe);
    for m in &markets {
        tracing::debug!(
            "Market: {} YES={} TimeRemaining={}s",
            m.question, m.yes_price, m.time_remaining
        );
    }

    markets
}

/// API endpoint: Get active BTC/crypto markets
pub async fn get_active_markets(
    Query(query): Query<ActiveMarketsQuery>,
) -> Response {
    let timeframe = query.timeframe.unwrap_or_else(|| "5".to_string());
    let asset_filter = query.asset;

    let markets = fetch_active_markets(&timeframe).await;

    // Filter by asset if specified
    let filtered = if let Some(asset) = asset_filter {
        markets
            .into_iter()
            .filter(|m| m.asset.to_lowercase() == asset.to_lowercase())
            .collect()
    } else {
        markets
    };

    Json(ActiveMarketsResponse {
        success: true,
        markets: filtered.clone(),
        count: filtered.len(),
        error: None,
    }).into_response()
}

/// API endpoint: Get current BTC price from Binance
pub async fn get_btc_price() -> Response {
    let client = reqwest::Client::new();

    match client
        .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            #[derive(Deserialize)]
            struct BinancePrice {
                price: String,
            }

            if let Ok(data) = resp.json::<BinancePrice>().await {
                if let Ok(price) = data.price.parse::<f64>() {
                    return Json(BitcoinPriceResponse {
                        success: true,
                        price: Some(price),
                        error: None,
                    }).into_response();
                }
            }
        }
        _ => {}
    }

    Json(BitcoinPriceResponse {
        success: false,
        price: None,
        error: Some("Failed to fetch BTC price".to_string()),
    }).into_response()
}

/// API endpoint: Get Polymarket YES/NO price for a token
pub async fn get_market_price(
    Query(payload): Query<MarketPriceRequest>,
) -> Response {
    let token_id = payload.token_id.unwrap_or_default();

    if token_id.is_empty() {
        return Json(MarketPriceResponse {
            success: false,
            yes_price: None,
            no_price: None,
            token_id: None,
            error: Some("token_id required".to_string()),
        }).into_response();
    }

    let client = reqwest::Client::new();

    // Try midpoint endpoint first (fastest)
    let midpoint_url = format!("{}/midpoint?token_id={token_id}", CLOB_API);
    if let Ok(resp) = client.get(&midpoint_url).timeout(std::time::Duration::from_millis(800)).send().await {
        if resp.status().is_success() {
            #[derive(Deserialize)]
            struct MidpointResponse {
                mid: String,
            }

            if let Ok(data) = resp.json::<MidpointResponse>().await {
                if let Ok(price) = data.mid.parse::<f64>() {
                    if price > 0.0 && price < 1.0 {
                        return Json(MarketPriceResponse {
                            success: true,
                            yes_price: Some(price),
                            no_price: Some(1.0 - price),
                            token_id: Some(token_id),
                            error: None,
                        }).into_response();
                    }
                }
            }
        }
    }

    // Try book endpoint as fallback
    let book_url = format!("{}/book?token_id={token_id}", CLOB_API);
    if let Ok(resp) = client.get(&book_url).timeout(std::time::Duration::from_millis(1000)).send().await {
        if resp.status().is_success() {
            #[derive(Deserialize)]
            struct BookResponse {
                #[serde(default)]
                bids: Option<Vec<PriceLevel>>,
                #[serde(default)]
                asks: Option<Vec<PriceLevel>>,
                #[serde(default)]
                last_trade_price: Option<String>,
            }

            #[derive(Deserialize)]
            struct PriceLevel {
                price: String,
            }

            if let Ok(book) = resp.json::<BookResponse>().await {
                // Calculate midpoint from best bid and ask
                let best_bid = book.bids.as_ref()
                    .and_then(|b| b.first())
                    .and_then(|b| b.price.parse::<f64>().ok());
                let best_ask = book.asks.as_ref()
                    .and_then(|a| a.first())
                    .and_then(|a| a.price.parse::<f64>().ok());

                if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                    let midpoint = (bid + ask) / 2.0;
                    return Json(MarketPriceResponse {
                        success: true,
                        yes_price: Some(midpoint),
                        no_price: Some(1.0 - midpoint),
                        token_id: Some(token_id),
                        error: None,
                    }).into_response();
                }

                // Fallback to last trade price
                if let Some(last_price) = book.last_trade_price {
                    if let Ok(price) = last_price.parse::<f64>() {
                        return Json(MarketPriceResponse {
                            success: true,
                            yes_price: Some(price),
                            no_price: Some(1.0 - price),
                            token_id: Some(token_id),
                            error: None,
                        }).into_response();
                    }
                }
            }
        }
    }

    Json(MarketPriceResponse {
        success: false,
        yes_price: None,
        no_price: None,
        token_id: Some(token_id),
        error: Some("Could not fetch price".to_string()),
    }).into_response()
}

/// API endpoint: List available markets (legacy endpoint)
pub async fn list_markets() -> Response {
    let client = reqwest::Client::new();

    match client
        .get("https://clob.polymarket.com/markets?limit=50")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if let Some(markets) = data.get("data").and_then(|d| d.as_array()) {
                    let market_list: Vec<MarketDetail> = markets.iter()
                        .filter_map(|m| {
                            Some(MarketDetail {
                                condition_id: m.get("condition_id")?.as_str()?.to_string(),
                                question: m.get("question")?.as_str()?.to_string(),
                                description: m.get("description").and_then(|d| d.as_str()).map(|s| s.to_string()),
                                yes_token_id: m.get("tokens")
                                    .and_then(|t| t.as_array())
                                    .and_then(|t| t.iter().find(|tok| {
                                        tok.get("outcome").and_then(|o| o.as_str()) == Some("Yes")
                                    }))
                                    .and_then(|t| t.get("token_id"))
                                    .and_then(|id| id.as_str())
                                    .map(|s| s.to_string()),
                                no_token_id: m.get("tokens")
                                    .and_then(|t| t.as_array())
                                    .and_then(|t| t.iter().find(|tok| {
                                        tok.get("outcome").and_then(|o| o.as_str()) == Some("No")
                                    }))
                                    .and_then(|t| t.get("token_id"))
                                    .and_then(|id| id.as_str())
                                    .map(|s| s.to_string()),
                                yes_price: None,
                                no_price: None,
                                volume: None,
                                liquidity: None,
                                end_date: m.get("end_date_iso").and_then(|d| d.as_str()).map(|s| s.to_string()),
                                price_to_beat: None,
                            })
                        })
                        .take(20)
                        .collect();

                    return Json(MarketListResponse {
                        success: true,
                        markets: market_list,
                        error: None,
                    }).into_response();
                }
            }
        }
        _ => {}
    }

    Json(MarketListResponse {
        success: false,
        markets: vec![],
        error: Some("Failed to fetch markets".to_string()),
    }).into_response()
}

/// API endpoint: Get market by condition_id
pub async fn get_market_by_condition(condition_id: &str) -> Option<MarketDetail> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/markets/{condition_id}", CLOB_API))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            #[derive(Deserialize)]
            struct MarketData {
                condition_id: String,
                question: String,
                #[serde(default)]
                description: Option<String>,
                #[serde(default)]
                tokens: Option<Vec<TokenInfo>>,
                #[serde(default)]
                outcome_prices: Option<String>,
                #[serde(default)]
                volume_num: Option<f64>,
                #[serde(default)]
                liquidity: Option<f64>,
                #[serde(rename = "endDate", default)]
                end_date: Option<String>,
            }

            #[derive(Deserialize)]
            struct TokenInfo {
                token_id: Option<String>,
                outcome: Option<String>,
            }

            let market: MarketData = resp.json().await.ok()?;

            let tokens = market.tokens.as_ref();
            let yes_token_id = tokens
                .and_then(|t| t.iter().find(|tok| tok.outcome.as_deref() == Some("Yes")))
                .and_then(|t| t.token_id.clone());
            let no_token_id = tokens
                .and_then(|t| t.iter().find(|tok| tok.outcome.as_deref() == Some("No")))
                .and_then(|t| t.token_id.clone());

            let (yes_price, no_price) = market.outcome_prices
                .as_ref()
                .and_then(|p| {
                    let prices: Vec<String> = serde_json::from_str(p).ok()?;
                    let yes = prices.first()?.parse::<f64>().ok()?;
                    Some((yes, 1.0 - yes))
                })
                .unwrap_or((0.5, 0.5));

            Some(MarketDetail {
                condition_id: market.condition_id,
                question: market.question,
                description: market.description,
                yes_token_id,
                no_token_id,
                yes_price: Some(yes_price),
                no_price: Some(no_price),
                volume: market.volume_num,
                liquidity: market.liquidity,
                end_date: market.end_date,
                price_to_beat: None,
            })
        }
        _ => None,
    }
}
