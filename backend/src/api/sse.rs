//! Server-Sent Events for real-time bot and market updates

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::AppState;
use crate::trading::orchestrator::BotEvent;

const GAMMA_API: &str = "https://gamma-api.polymarket.com";
const CLOB_API: &str = "https://clob.polymarket.com";
const BINANCE_API: &str = "https://api.binance.com/api/v3";
const TIMEFRAME_DURATION_SECS: i64 = 300; // 5 minutes

/// Fetch current BTC price from Binance
async fn fetch_btc_price(client: &reqwest::Client) -> Option<f64> {
    let result = client
        .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            #[derive(serde::Deserialize)]
            struct BinancePrice {
                price: String,
            }

            match resp.json::<BinancePrice>().await {
                Ok(data) => data.price.parse::<f64>().ok(),
                Err(_) => None,
            }
        }
        _ => None,
    }
}

/// Fetch historical BTC price at specific timestamp from Binance (1m kline open price)
async fn fetch_btc_price_at_timestamp(client: &reqwest::Client, timestamp: i64) -> Option<f64> {
    let start_time_ms = timestamp * 1000;

    let result = client
        .get(format!(
            "{}/klines?symbol=BTCUSDT&interval=1m&startTime={}&limit=1",
            BINANCE_API, start_time_ms
        ))
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Vec<Vec<serde_json::Value>>>().await {
                Ok(klines) if !klines.is_empty() => {
                    // Kline format: [open_time, open, high, low, close, ...]
                    klines[0].get(1)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Fetch market by slug using timestamp-based discovery
async fn fetch_market_by_slug(client: &reqwest::Client, slug: &str) -> Option<(serde_json::Value, String)> {

    let result = client
        .get(format!("{}/events/slug/{}", GAMMA_API, slug))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(event) => {
                    // Check if event is active
                    if event.get("active").and_then(|a| a.as_bool()) != Some(true) {
                        return None;
                    }
                    if event.get("closed").and_then(|c| c.as_bool()) == Some(true) {
                        return None;
                    }

                    // Get first market
                    let markets = event.get("markets").and_then(|m| m.as_array())?;
                    let market = markets.first()?.clone();

                    // Check if market is active
                    if market.get("active").and_then(|a| a.as_bool()) != Some(true) {
                        return None;
                    }
                    if market.get("closed").and_then(|c| c.as_bool()) == Some(true) {
                        return None;
                    }

                    // Check if not expired
                    let end_date = market.get("endDate").and_then(|d| d.as_str())?;
                    let end_time = chrono::DateTime::parse_from_rfc3339(end_date)
                        .ok()
                        .map(|dt| dt.timestamp())?;

                    if end_time < chrono::Utc::now().timestamp() {
                        return None;
                    }

                    // Extract clobTokenIds for fast midpoint fetching
                    let clob_token_ids = market.get("clobTokenIds")
                        .and_then(|t| t.as_str())
                        .and_then(|s| {
                            let ids: Vec<String> = serde_json::from_str(s).ok()?;
                            if ids.len() >= 2 {
                                Some(ids[0].clone()) // First token is usually YES/UP
                            } else {
                                None
                            }
                        });

                    // Store token ID in market data
                    let mut market_data = market.clone();
                    if let Some(token_id) = clob_token_ids {
                        market_data["yes_token_id"] = serde_json::json!(token_id);
                    }

                    tracing::info!("Found active market via slug {}: {}", slug,
                        market.get("question").and_then(|q| q.as_str()).unwrap_or(""));

                    Some((market_data, slug.to_string()))
                }
                Err(_) => None,
            }
        }
        _ => None,
    }
}

/// Discover active BTC up/down markets using timestamp-based slugs
async fn discover_btc_market(client: &reqwest::Client) -> Option<(serde_json::Value, String)> {
    let now = chrono::Utc::now().timestamp();
    let rounded_time = (now / TIMEFRAME_DURATION_SECS) * TIMEFRAME_DURATION_SECS;

    // Try multiple offsets to handle timing mismatches
    for offset in 0..4 {
        let try_time = rounded_time - (offset * TIMEFRAME_DURATION_SECS);
        let slug = format!("btc-updown-5m-{}", try_time);

        if let Some(market) = fetch_market_by_slug(client, &slug).await {
            return Some(market);
        }
    }

    // Try ETH as fallback
    for offset in 0..4 {
        let try_time = rounded_time - (offset * TIMEFRAME_DURATION_SECS);
        let slug = format!("eth-updown-5m-{}", try_time);

        if let Some(market) = fetch_market_by_slug(client, &slug).await {
            tracing::info!("Using ETH market as fallback");
            return Some(market);
        }
    }

    tracing::warn!("No active BTC/ETH up/down market found");
    None
}

/// Fetch fast midpoint price from CLOB API (faster than Gamma)
async fn fetch_clob_midpoint(client: &reqwest::Client, token_id: &str) -> Option<f64> {

    let result = client
        .get(format!("{}/midpoint?token_id={}", CLOB_API, token_id))
        .timeout(std::time::Duration::from_millis(500))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    data.get("mid").and_then(|m| m.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .or_else(|| data.get("mid").and_then(|m| m.as_f64()))
                }
                Err(_) => None,
            }
        }
        _ => None,
    }
}

/// Fetch market prices from Gamma API (fallback)
async fn fetch_gamma_prices(client: &reqwest::Client, market_id: &str) -> Option<(f64, f64)> {

    let result = client
        .get(format!("{}/markets/{}", GAMMA_API, market_id))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(market) => {
                    let prices_str = market.get("outcomePrices").and_then(|p| p.as_str())?;
                    let prices: Vec<&str> = prices_str.split(',').collect();

                    if prices.len() >= 2 {
                        let yes_str = prices[0].trim().trim_matches('"').trim_matches('[').trim_matches('"');
                        let no_str = prices[1].trim().trim_matches('"').trim_matches(']').trim_matches('"');

                        let yes = yes_str.parse::<f64>().ok()?;
                        let no = no_str.parse::<f64>().ok()?;

                        Some((yes, no))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
        _ => None,
    }
}

/// SSE stream for bot and market events
pub async fn bot_events_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let state_clone = state.clone();

    // Shared reqwest client (connection pooling)
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    // Subscribe to bot events from broadcast channel
    let mut bot_event_subscriber = state.bot_event_broadcaster.subscribe();

    // Shared state
    let last_btc_price = Arc::new(RwLock::new(0.0));
    let last_start_price = Arc::new(RwLock::new(0.0)); // BTC price at market start time
    let last_market = Arc::new(RwLock::new(None::<serde_json::Value>));
    let last_market_id = Arc::new(RwLock::new(String::new()));
    let last_yes_token = Arc::new(RwLock::new(String::new()));
    let last_event_start_time = Arc::new(RwLock::new(0i64)); // Market start timestamp
    let last_api_latency = Arc::new(RwLock::new(0.0)); // Tracking API latency

    // Sequence counter for event ordering
    let seq = Arc::new(RwLock::new(0u64));

    tracing::info!("Starting SSE stream for real-time Polymarket updates");

    let stream = async_stream::stream! {
        // Initial connection message
        yield Ok(Event::default()
            .event("connected")
            .data(r#"{"type":"connected","message":"SSE connected"}"#));

        // Intervals
        let mut status_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        let mut btc_interval = tokio::time::interval(std::time::Duration::from_secs(2));
        let mut price_interval = tokio::time::interval(std::time::Duration::from_millis(300)); // Fast!
        let mut discovery_interval = tokio::time::interval(std::time::Duration::from_secs(5)); // Check every 5s

        // Skip first ticks
        status_interval.tick().await;
        btc_interval.tick().await;
        price_interval.tick().await;
        discovery_interval.tick().await;

        // Initial market discovery
        if let Some((market, slug)) = discover_btc_market(&http_client).await {
            let market_id = market.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
            let yes_token = market.get("yes_token_id").and_then(|t| t.as_str()).unwrap_or("").to_string();

            // Extract event_start_time from slug (format: btc-updown-5m-{timestamp})
            let event_start_time: i64 = slug
                .strip_prefix("btc-updown-5m-")
                .or_else(|| slug.strip_prefix("eth-updown-5m-"))
                .and_then(|ts| ts.parse::<i64>().ok())
                .unwrap_or(0);

            // Fetch HISTORICAL BTC price at market start time (this is the TRUE price_to_beat)
            let start_price = if event_start_time > 0 {
                fetch_btc_price_at_timestamp(&http_client, event_start_time).await.unwrap_or(0.0)
            } else {
                0.0
            };

            // Fetch current BTC price
            let current_btc = fetch_btc_price(&http_client).await.unwrap_or(0.0);

            let mut market_lock = last_market.write().await;
            *market_lock = Some(market.clone());
            let mut id_lock = last_market_id.write().await;
            *id_lock = market_id;
            let mut token_lock = last_yes_token.write().await;
            *token_lock = yes_token;
            let mut start_time_lock = last_event_start_time.write().await;
            *start_time_lock = event_start_time;

            // Update shared prices
            let mut btc_lock = last_btc_price.write().await;
            *btc_lock = current_btc;
            let mut start_price_lock = last_start_price.write().await;
            *start_price_lock = start_price;

            tracing::info!("Initial market: {} (event_start={}, price_to_beat=${:.2}, current_btc=${:.2})",
                market.get("question").and_then(|q| q.as_str()).unwrap_or(""),
                event_start_time, start_price, current_btc);
        }

        loop {
            tokio::select! {
                // Bot events from broadcast channel
                bot_event_result = bot_event_subscriber.recv() => {
                    match bot_event_result {
                        Ok(event) => {
                            let mut seq_lock = seq.write().await;
                            *seq_lock += 1;
                            let current_seq = *seq_lock;
                            let event_data = match &event {
                                BotEvent::SessionStarted { bot_id, session_id, bot_name } => {
                                    serde_json::json!({
                                        "type": "session_started",
                                        "bot_id": bot_id,
                                        "session_id": session_id,
                                        "bot_name": bot_name,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::SessionEnded { bot_id, session_id, final_balance, total_pnl } => {
                                    serde_json::json!({
                                        "type": "session_ended",
                                        "bot_id": bot_id,
                                        "session_id": session_id,
                                        "final_balance": final_balance,
                                        "total_pnl": total_pnl,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::TradeDecision { bot_id, outcome, confidence, bet_size, reason } => {
                                    serde_json::json!({
                                        "type": "trade_decision",
                                        "bot_id": bot_id,
                                        "outcome": outcome,
                                        "confidence": confidence,
                                        "bet_size": bet_size,
                                        "reason": reason,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::OrderExecuted { bot_id, order_id } => {
                                    serde_json::json!({
                                        "type": "order_executed",
                                        "bot_id": bot_id,
                                        "order_id": order_id,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::BalanceUpdated { bot_id, balance } => {
                                    serde_json::json!({
                                        "type": "balance_updated",
                                        "bot_id": bot_id,
                                        "balance": balance,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::Error { bot_id, message } => {
                                    serde_json::json!({
                                        "type": "error",
                                        "bot_id": bot_id,
                                        "message": message,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::MarketTransition { new_market_slug } => {
                                    serde_json::json!({
                                        "type": "market_transition",
                                        "new_market_slug": new_market_slug,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::Scanning { bot_id, market_slug } => {
                                    serde_json::json!({
                                        "type": "scanning",
                                        "bot_id": bot_id,
                                        "market_slug": market_slug,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::Evaluating { bot_id, strategy, confidence } => {
                                    serde_json::json!({
                                        "type": "evaluating",
                                        "bot_id": bot_id,
                                        "strategy": strategy,
                                        "confidence": confidence,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::PositionUpdate { bot_id, side, size, price, unrealized_pnl } => {
                                    serde_json::json!({
                                        "type": "position_update",
                                        "bot_id": bot_id,
                                        "side": side,
                                        "size": size,
                                        "price": price,
                                        "unrealized_pnl": unrealized_pnl,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                                BotEvent::TradeResult { bot_id, won, pnl } => {
                                    serde_json::json!({
                                        "type": "trade_result",
                                        "bot_id": bot_id,
                                        "won": won,
                                        "pnl": pnl,
                                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                                        "seq": current_seq
                                    })
                                }
                            };

                            yield Ok(Event::default()
                                .event("bot")
                                .data(event_data.to_string()));
                        }
                        Err(e) => {
                            tracing::warn!("Bot event broadcast error: {}", e);
                        }
                    }
                }

                // Market discovery (every 5s)
                _ = discovery_interval.tick() => {
                    let current_market = last_market.read().await.clone();
                    let needs_new = if let Some(ref m) = current_market {
                        let end_date = m.get("endDate").and_then(|d| d.as_str());
                        end_date.is_none_or(|end| {
                            chrono::DateTime::parse_from_rfc3339(end)
                                .ok()
                                .is_none_or(|dt| dt.timestamp() < chrono::Utc::now().timestamp())
                        })
                    } else {
                        true
                    };

                    if needs_new {
                        tracing::info!("Discovering new market...");
                        if let Some((market, _slug)) = discover_btc_market(&http_client).await {
                            let market_id = market.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                            let yes_token = market.get("yes_token_id").and_then(|t| t.as_str()).unwrap_or("").to_string();

                            // Get eventStartTime - the start of the 5-minute window
                            let event_start_time = market.get("eventStartTime")
                                .and_then(|t| t.as_str())
                                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                                .map(|dt| dt.timestamp())
                                .unwrap_or(0);

                            let mut market_lock = last_market.write().await;
                            *market_lock = Some(market.clone());
                            let mut id_lock = last_market_id.write().await;
                            *id_lock = market_id;
                            let mut token_lock = last_yes_token.write().await;
                            *token_lock = yes_token;
                            let mut start_time_lock = last_event_start_time.write().await;
                            *start_time_lock = event_start_time;
                            // Reset start price when new market is discovered
                            let mut start_price_lock = last_start_price.write().await;
                            *start_price_lock = 0.0;

                            tracing::info!("New market: {} (starts at {})",
                                market.get("question").and_then(|q| q.as_str()).unwrap_or(""),
                                event_start_time);
                        }
                    }
                }

                // Fast price update (every 300ms) - use CLOB midpoint for fastest prices
                _ = price_interval.tick() => {
                    let market_id = last_market_id.read().await.clone();
                    let yes_token = last_yes_token.read().await.clone();
                    let market = last_market.read().await.clone();
                    let event_start_time = *last_event_start_time.read().await;
                    let current_time = chrono::Utc::now().timestamp();

                    if !market_id.is_empty() {
                        // Try CLOB API first (fastest)
                        let (yes, no) = if !yes_token.is_empty() {
                            let start_time = tokio::time::Instant::now();
                            if let Some(yes_price) = fetch_clob_midpoint(&http_client, &yes_token).await {
                                let latency = start_time.elapsed().as_millis() as f64;
                                *last_api_latency.write().await = latency;
                                (yes_price, 1.0 - yes_price)
                            } else {
                                // Fallback to Gamma
                                let start_time = tokio::time::Instant::now();
                                let prices = fetch_gamma_prices(&http_client, &market_id).await.unwrap_or((0.5, 0.5));
                                let latency = start_time.elapsed().as_millis() as f64;
                                *last_api_latency.write().await = latency;
                                prices
                            }
                        } else {
                            let start_time = tokio::time::Instant::now();
                            let prices = fetch_gamma_prices(&http_client, &market_id).await.unwrap_or((0.5, 0.5));
                            let latency = start_time.elapsed().as_millis() as f64;
                            *last_api_latency.write().await = latency;
                            prices
                        };

                        let btc_price = *last_btc_price.read().await;
                        let current_start_price = *last_start_price.read().await;

                        // Capture start price when market begins (only once per market)
                        // If we connect mid-market, use current price as baseline
                        if btc_price > 0.0 && event_start_time > 0 && current_start_price == 0.0 {
                            let mut start_price_lock = last_start_price.write().await;
                            *start_price_lock = btc_price;
                            tracing::info!("Captured start price: {} for market starting at {}", btc_price, event_start_time);
                        }

                        let start_price = *last_start_price.read().await;

                        // Calculate time remaining until market end
                        let time_remaining = market.as_ref()
                            .and_then(|m| m.get("endDate").and_then(|d| d.as_str()))
                            .and_then(|end| chrono::DateTime::parse_from_rfc3339(end).ok())
                            .map_or(300, |dt| dt.timestamp() - current_time)
                            .max(0);

                        // Extract volume and question (try multiple Gamma API field names)
                        let volume = market.as_ref()
                            .and_then(|m| {
                                m.get("volume_24hr").and_then(|v| v.as_f64())
                                    .or_else(|| m.get("volume").and_then(|v| v.as_f64()))
                                    .or_else(|| m.get("volumeNum").and_then(|v| v.as_f64()))
                                    .or_else(|| m.get("liquidityNum").and_then(|v| v.as_f64()))
                                    .or_else(|| m.get("liquidity").and_then(|v| v.as_f64()))
                            })
                            .unwrap_or(0.0);

                        let question = market.as_ref()
                            .and_then(|m| m.get("question").and_then(|q| q.as_str()))
                            .unwrap_or("BTC Up or Down?");

                        // Determine market sentiment based on YES price
                        let sentiment = if yes > 0.5 { "UP" } else { "DOWN" };

                        // Calculate price delta (current vs start)
                        let price_delta = if start_price > 0.0 && btc_price > 0.0 {
                            btc_price - start_price
                        } else {
                            0.0
                        };

                        let mut seq_lock = seq.write().await;
                        *seq_lock += 1;
                        let current_seq = *seq_lock;
                        let server_ts = chrono::Utc::now().timestamp_millis();

                        // Only include start_price/price_to_beat/price_delta once start_price has been captured
                        // This prevents flickering on market transitions where start_price is temporarily 0
                        let update = if start_price > 0.0 {
                            serde_json::json!({
                                "type": "market_price",
                                "btc_price": btc_price,
                                "start_price": start_price,
                                "price_to_beat": start_price,
                                "price_delta": price_delta,
                                "yes": yes,
                                "no": no,
                                "time_remaining": time_remaining,
                                "market_duration": 300,
                                "volume": volume,
                                "market_question": question,
                                "sentiment": sentiment,
                                "event_start_time": event_start_time,
                                "api_latency": *last_api_latency.read().await,
                                "server_timestamp": server_ts,
                                "seq": current_seq
                            })
                        } else {
                            // start_price not yet captured - don't send it to avoid flickering
                            serde_json::json!({
                                "type": "market_price",
                                "btc_price": btc_price,
                                "yes": yes,
                                "no": no,
                                "time_remaining": time_remaining,
                                "market_duration": 300,
                                "volume": volume,
                                "market_question": question,
                                "sentiment": sentiment,
                                "event_start_time": event_start_time,
                                "api_latency": *last_api_latency.read().await,
                                "server_timestamp": server_ts,
                                "seq": current_seq
                            })
                        };

                        yield Ok(Event::default()
                            .event("market")
                            .data(update.to_string()));
                    }
                }

                // BTC price update (every 2s)
                _ = btc_interval.tick() => {
                    let start_time = tokio::time::Instant::now();
                    if let Some(btc_price) = fetch_btc_price(&http_client).await {
                        let latency = start_time.elapsed().as_millis() as f64;
                        let mut latency_lock = last_api_latency.write().await;
                        // Average with existing latency
                        if *latency_lock > 0.0 {
                            *latency_lock = (*latency_lock + latency) / 2.0;
                        } else {
                            *latency_lock = latency;
                        }
                        
                        if btc_price > 0.0 {
                            let mut price_lock = last_btc_price.write().await;
                            *price_lock = btc_price;
                        }
                    }
                }

                // Status update (every 5s)
                _ = status_interval.tick() => {
                    let running_bots = state_clone.orchestrator.get_all_running_bots().await.len();
                    let btc_price = *last_btc_price.read().await;
                    let mut seq_lock = seq.write().await;
                    *seq_lock += 1;
                    let current_seq = *seq_lock;

                    let status = serde_json::json!({
                        "type": "status",
                        "running_bots": running_bots,
                        "btc_price": btc_price,
                        "total_pnl": 0.0,
                        "server_timestamp": chrono::Utc::now().timestamp_millis(),
                        "seq": current_seq
                    });

                    yield Ok(Event::default()
                        .event("status")
                        .data(status.to_string()));
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
