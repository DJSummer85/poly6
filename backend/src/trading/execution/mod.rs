//! Execution adapters for demo and live trading
pub mod paper;
pub mod live;

pub use paper::PaperExecutionAdapter;
pub use live::LiveExecutionAdapter;
