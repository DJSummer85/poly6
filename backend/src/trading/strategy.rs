use serde::{Deserialize, Serialize};

use super::client::{ClobClient, OrderRequest, OrderResponse};
use super::websocket::{get_mid_price, get_spread, OrderBookClient};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyParams {
    pub market_id: String,
    pub min_size: f64,          // Minimum order size
    pub max_size: f64,         // Maximum order size
    pub spread_threshold: f64, // Minimum spread to trade
    pub price_diff_threshold: f64, // Price movement threshold
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            market_id: "btc_5min".to_string(),
            min_size: 1.0,
            max_size: 10.0,
            spread_threshold: 0.01,
            price_diff_threshold: 0.005,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Signal {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyState {
    pub last_price: Option<f64>,
    pub last_signal: Signal,
    pub position_size: f64,
    pub orders_placed: u32,
    pub pnl: f64,
}

impl Default for StrategyState {
    fn default() -> Self {
        Self {
            last_price: None,
            last_signal: Signal::Hold,
            position_size: 0.0,
            orders_placed: 0,
            pnl: 0.0,
        }
    }
}

/// Simple BTC 5-minute strategy based on order book analysis
pub struct StrategyExecutor {
    params: StrategyParams,
    state: StrategyState,
    orderbook_client: OrderBookClient,
    clob_client: ClobClient,
}

impl StrategyExecutor {
    pub fn new(params: StrategyParams, api_key: Option<String>) -> Self {
        Self {
            params: params.clone(),
            state: StrategyState::default(),
            orderbook_client: OrderBookClient::new(&params.market_id),
            clob_client: ClobClient::new(api_key),
        }
    }

    /// Evaluate the current market and return a trading signal
    pub async fn evaluate(&mut self) -> Result<Signal, String> {
        // Subscribe to order book
        let mut stream = self.orderbook_client.subscribe().await?;

        // Get latest order book
        let order_book = match stream.next_orderbook().await {
            Some(ob) => ob,
            None => return Ok(Signal::Hold),
        };

        let mid_price = match get_mid_price(&order_book) {
            Some(price) => price,
            None => return Ok(Signal::Hold),
        };

        let spread = get_spread(&order_book).unwrap_or(999.0);

        // Check if spread is wide enough
        if spread > self.params.spread_threshold {
            return Ok(Signal::Hold);
        }

        // Calculate price change from last signal
        let price_change = if let Some(last_price) = self.state.last_price {
            (mid_price - last_price) / last_price
        } else {
            0.0
        };

        // Simple momentum strategy:
        // - Buy if price increased significantly (> threshold)
        // - Sell if price decreased significantly (< -threshold)
        // - Hold otherwise

        let signal = if price_change > self.params.price_diff_threshold {
            Signal::Buy
        } else if price_change < -self.params.price_diff_threshold {
            Signal::Sell
        } else {
            Signal::Hold
        };

        self.state.last_price = Some(mid_price);
        self.state.last_signal = signal.clone();

        Ok(signal)
    }

    /// Execute a trade based on the signal
    pub async fn execute(&mut self, signal: Signal) -> Result<Option<OrderResponse>, String> {
        if signal == Signal::Hold {
            return Ok(None);
        }

        let mid_price = self.state.last_price.unwrap_or(0.0);

        let side = match signal {
            Signal::Buy => "buy",
            Signal::Sell => "sell",
            Signal::Hold => return Ok(None),
        };

        let order = OrderRequest {
            market: self.params.market_id.clone(),
            side: side.to_string(),
            price: mid_price,
            size: self.params.min_size,
            order_type: "GTC".to_string(),
        };

        match self.clob_client.place_order(&order).await {
            Ok(response) => {
                self.state.orders_placed += 1;
                tracing::info!(
                    "Order placed: {} {} {} @ {}",
                    side,
                    self.params.min_size,
                    self.params.market_id,
                    mid_price
                );
                Ok(Some(response))
            }
            Err(e) => {
                tracing::error!("Order failed: {}", e);
                Err(e)
            }
        }
    }

    /// Run one iteration of the strategy
    pub async fn tick(&mut self) -> Result<Option<OrderResponse>, String> {
        let signal = self.evaluate().await?;

        if signal != Signal::Hold {
            self.execute(signal).await
        } else {
            Ok(None)
        }
    }

    pub fn get_state(&self) -> &StrategyState {
        &self.state
    }

    pub fn reset_state(&mut self) {
        self.state = StrategyState::default();
    }
}
