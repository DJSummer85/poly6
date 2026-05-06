use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};

type WsStream = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type WsSplitStream = futures_util::stream::SplitStream<WsStream>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub market_id: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "order_book")]
    OrderBook {
        market: String,
        bids: Vec<Vec<f64>>,
        asks: Vec<Vec<f64>>,
    },
    #[serde(rename = "price_change")]
    PriceChange {
        market: String,
        price: f64,
        side: String,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

pub struct OrderBookClient {
    ws_url: String,
    market_id: String,
}

impl OrderBookClient {
    pub fn new(market_id: &str) -> Self {
        let ws_url = "wss://clob.polymarket.com/ws";

        Self {
            ws_url: ws_url.to_string(),
            market_id: market_id.to_string(),
        }
    }

    pub async fn subscribe(&self) -> Result<OrderBookStream, String> {
        let (ws_stream, _) = connect_async(&self.ws_url)
            .await
            .map_err(|e| format!("WebSocket connection failed: {}", e))?;

        let (mut write, read) = ws_stream.split();

        let subscribe_msg = serde_json::json!({
            "type": "subscribe",
            "channel": "order_book",
            "market": self.market_id
        });

        write
            .send(Message::Text(subscribe_msg.to_string()))
            .await
            .map_err(|e| format!("Failed to send subscription: {}", e))?;

        Ok(OrderBookStream {
            stream: read,
            market_id: self.market_id.clone(),
        })
    }
}

pub struct OrderBookStream {
    stream: WsSplitStream,
    market_id: String,
}

impl OrderBookStream {
    pub async fn next_orderbook(&mut self) -> Option<OrderBook> {
        while let Some(msg) = self.stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(ws_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_type) = ws_msg.get("type").and_then(|v| v.as_str()) {
                            if msg_type == "order_book" {
                                let market = ws_msg.get("market")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&self.market_id)
                                    .to_string();

                                let bids: Vec<OrderBookLevel> = ws_msg
                                    .get("bids")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| {
                                                let arr = v.as_array()?;
                                                let price = arr.first()?.as_f64()?;
                                                let size = arr.get(1)?.as_f64()?;
                                                Some(OrderBookLevel { price, size })
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                let asks: Vec<OrderBookLevel> = ws_msg
                                    .get("asks")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| {
                                                let arr = v.as_array()?;
                                                let price = arr.first()?.as_f64()?;
                                                let size = arr.get(1)?.as_f64()?;
                                                Some(OrderBookLevel { price, size })
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                return Some(OrderBook {
                                    bids,
                                    asks,
                                    market_id: market,
                                    timestamp: chrono::Utc::now().timestamp(),
                                });
                            }
                        }
                    }
                }
                Ok(Message::Ping(_)) => {}
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        None
    }
}

pub fn get_mid_price(order_book: &OrderBook) -> Option<f64> {
    let best_bid = order_book.bids.first().map(|b| b.price);
    let best_ask = order_book.asks.first().map(|a| a.price);

    match (best_bid, best_ask) {
        (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
        _ => None,
    }
}

pub fn get_spread(order_book: &OrderBook) -> Option<f64> {
    let best_bid = order_book.bids.first().map(|b| b.price);
    let best_ask = order_book.asks.first().map(|a| a.price);

    match (best_bid, best_ask) {
        (Some(bid), Some(ask)) => Some(ask - bid),
        _ => None,
    }
}
