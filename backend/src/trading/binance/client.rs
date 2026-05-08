use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::ticker::{PriceUpdate, Ticker};
use super::kline::{KlineMessage, KlineUpdate};

const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";

#[derive(Debug, Clone)]
pub struct BinanceClient {
    symbol: String,
    price_tx: broadcast::Sender<PriceUpdate>,
    kline_tx: broadcast::Sender<KlineUpdate>,
    running: Arc<RwLock<bool>>,
    current_price: Arc<RwLock<Option<f64>>>, // Cache current price
}

impl BinanceClient {
    pub fn new(symbol: &str) -> Self {
        let (price_tx, _) = broadcast::channel(100);
        let (kline_tx, _) = broadcast::channel(100);

        Self {
            symbol: symbol.to_uppercase(),
            price_tx,
            kline_tx,
            running: Arc::new(RwLock::new(false)),
            current_price: Arc::new(RwLock::new(None)),
        }
    }

    /// Subscribe to price updates (ticker stream)
    pub fn subscribe_price(&self) -> broadcast::Receiver<PriceUpdate> {
        self.price_tx.subscribe()
    }

    /// Subscribe to kline (candle) updates
    pub fn subscribe_kline(&self) -> broadcast::Receiver<KlineUpdate> {
        self.kline_tx.subscribe()
    }

    /// Get current cached price
    pub async fn get_current_price(&self) -> Option<f64> {
        *self.current_price.read().await
    }

    /// Start the WebSocket connections
    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        let symbol_lower = self.symbol.to_lowercase();
        let ticker_stream = format!("{}@ticker", symbol_lower);
        let kline_stream = format!("{}@kline_1s", symbol_lower);

        // Connect to combined stream (ticker + kline)
        let streams = format!("{}/{}", ticker_stream, kline_stream);
        let url = format!("{}/{}", BINANCE_WS_URL, streams);

        tracing::info!("Connecting to Binance WebSocket: {}", url);

        let (ws, _) = connect_async(&url)
            .await
            .map_err(|e| format!("Failed to connect to Binance: {}", e))?;

        let (mut write, mut read) = ws.split();

        // Clone for the task
        let running = self.running.clone();
        let price_tx = self.price_tx.clone();
        let kline_tx = self.kline_tx.clone();
        let current_price = self.current_price.clone();

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                let is_running = *running.read().await;
                if !is_running {
                    break;
                }

                match msg {
                    Ok(Message::Text(text)) => {
                        // Try to parse as ticker first
                        if let Ok(ticker) = serde_json::from_str::<Ticker>(&text) {
                            let update = PriceUpdate::from(&ticker);
                            // Cache the price
                            {
                                let mut price = current_price.write().await;
                                *price = Some(update.price);
                            }
                            let _ = price_tx.send(update);
                        }
                        // Try to parse as kline
                        else if let Ok(kline_msg) = serde_json::from_str::<KlineMessage>(&text) {
                            let update = KlineUpdate::from(&kline_msg.kline);
                            let _ = kline_tx.send(update);
                        }
                    }
                    Ok(Message::Ping(ping)) => {
                        let _ = write.send(Message::Pong(ping)).await;
                    }
                    Ok(Message::Close(_)) => {
                        tracing::warn!("Binance WebSocket closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Binance WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            // Clear running flag when loop exits
            let mut r = running.write().await;
            *r = false;
            tracing::info!("Binance WebSocket stopped");
        });

        tracing::info!("Binance WebSocket connected for {}", self.symbol);
        Ok(())
    }

    /// Stop the WebSocket connections
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Check if client is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

// Convenience function to get BTC price updates
pub fn btc_price_stream() -> BinanceClient {
    BinanceClient::new("btcusdt")
}
