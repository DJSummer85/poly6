//! Strategy module - Trading strategies for Polymarket
//! Ported from polymarket-demo TypeScript implementation
//!
//! Each strategy is in its own file for clarity and maintainability

pub mod base;
pub mod momentum;
pub mod window_delta;
pub mod fair_value;
pub mod contrarian;
pub mod mean_reversion;
pub mod trend;
pub mod volatility;
pub mod sniper;
pub mod oracle_lag;
pub mod binance_velocity;

// Re-exports
pub use base::{Strategy, StrategyDecision, Signal, StrategyParams};
