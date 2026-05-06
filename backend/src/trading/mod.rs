pub mod binance;
pub mod bot_executor;
pub mod client;
pub mod orchestrator;
pub mod polymarket;
pub mod position;
pub mod strategy;
pub mod websocket;
pub mod risk_manager;
pub mod strategy_coordinator;
pub mod bot_loss_tracker;
pub mod market_data;
pub mod execution;
pub mod settlement;

pub use binance::client::btc_price_stream;
pub use binance::client::BinanceClient;
pub use polymarket::{check_matic_balance, PolymarketClient};
