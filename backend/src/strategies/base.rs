//! Strategy base types and helper functions
//! Common utilities used by all strategies

use serde::{Deserialize, Serialize};

/// Trading signal/action
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Signal {
    Yes,  // Buy/Long - betting that outcome will happen
    No,   // Sell/Short - betting against outcome
    Hold, // No action
}

impl Signal {
    pub fn is_trade(&self) -> bool {
        matches!(self, Signal::Yes | Signal::No)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Signal::Yes => "YES",
            Signal::No => "NO",
            Signal::Hold => "HOLD",
        }
    }
}

/// Strategy decision output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDecision {
    pub signal: Signal,
    pub confidence: f64,
    pub reason: String,
}

impl StrategyDecision {
    pub fn hold(reason: &str) -> Self {
        Self {
            signal: Signal::Hold,
            confidence: 0.0,
            reason: reason.to_string(),
        }
    }

    pub fn trade(signal: Signal, confidence: f64, reason: &str) -> Self {
        Self {
            signal,
            confidence: confidence.clamp(0.0, 1.0),
            reason: reason.to_string(),
        }
    }
}

/// Strategy parameters/thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyParams {
    pub min_delta: f64,
    pub max_delta: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub min_time_remaining: i64,
    pub max_time_remaining: i64,
    pub min_odds: Option<f64>,
    pub max_odds: Option<f64>,
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            min_delta: 0.02,        // 2% - reasonable threshold, not too noisy
            max_delta: 5.0,
            min_price: 0.25,        // Don't trade extreme odds
            max_price: 0.75,
            min_time_remaining: 5,
            max_time_remaining: 300,
            min_odds: None,
            max_odds: None,
        }
    }
}

/// Strategy context - data provided to each strategy
#[derive(Debug, Clone)]
pub struct StrategyContext {
    pub btc_price: Option<f64>,
    pub btc_price_change: Option<f64>,
    pub btc_window_open: Option<f64>,
    pub polymarket_price: Option<f64>,
    pub time_remaining: i64,
    pub order_book_spread: Option<f64>,
}

/// Strategy trait - each strategy implements this
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision;
}

/// Helper: Calculate BTC delta percentage
pub fn calculate_delta(current_price: f64, window_open: f64) -> f64 {
    if window_open <= 0.0 {
        return 0.0;
    }
    ((current_price - window_open) / window_open) * 100.0
}

/// Helper: Calculate fair probability from delta using tanh calibration
/// delta_pct should be a ratio (e.g. 0.002 for +0.2%), NOT a percentage
/// Returns a value in [0.05, 0.95] — a true probability
pub fn calculate_fair_prob(delta_pct: f64) -> f64 {
    0.5 + (delta_pct / 0.05).tanh() * 0.45
}

/// Helper: Check if we have sufficient edge over the market price
/// our_prob: our calculated fair probability
/// market_prob: Polymarket implied probability (YES price)
/// min_edge: minimum required edge (e.g. 0.07 for 7%)
pub fn has_sufficient_edge(our_prob: f64, market_prob: f64, min_edge: f64) -> bool {
    (our_prob - market_prob).abs() >= min_edge
}

/// Helper: Check if price is within acceptable range
pub fn check_price_limits(price: f64, params: &StrategyParams) -> bool {
    price >= params.min_price && price <= params.max_price
}

/// Helper: Check time remaining
pub fn check_time_remaining(time_remaining: i64, params: &StrategyParams) -> bool {
    time_remaining >= params.min_time_remaining && time_remaining <= params.max_time_remaining
}

/// Helper: Check delta threshold
pub fn check_delta(delta_pct: f64, params: &StrategyParams, direction: Option<&str>) -> bool {
    match direction {
        Some("up") => delta_pct > params.min_delta,
        Some("down") => delta_pct < -params.min_delta,
        _ => delta_pct.abs() > params.min_delta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_is_trade() {
        assert!(Signal::Yes.is_trade());
        assert!(Signal::No.is_trade());
        assert!(!Signal::Hold.is_trade());
    }

    #[test]
    fn test_signal_as_str() {
        assert_eq!(Signal::Yes.as_str(), "YES");
        assert_eq!(Signal::No.as_str(), "NO");
        assert_eq!(Signal::Hold.as_str(), "HOLD");
    }

    #[test]
    fn test_strategy_decision_hold() {
        let decision = StrategyDecision::hold("not enough data");
        assert!(matches!(decision.signal, Signal::Hold));
        assert_eq!(decision.confidence, 0.0);
        assert_eq!(decision.reason, "not enough data");
    }

    #[test]
    fn test_strategy_decision_trade_clamps_confidence() {
        let decision = StrategyDecision::trade(Signal::Yes, 1.5, "strong momentum");
        assert_eq!(decision.confidence, 1.0); // clamped from 1.5

        let decision = StrategyDecision::trade(Signal::No, -0.2, "rejection");
        assert_eq!(decision.confidence, 0.0); // clamped from -0.2
    }

    #[test]
    fn test_calculate_delta() {
        let delta = calculate_delta(80000.0, 79000.0);
        assert!((delta - 1.2658).abs() < 0.01);

        let delta = calculate_delta(78000.0, 79000.0);
        assert!((delta - (-1.2658)).abs() < 0.01);
    }

    #[test]
    fn test_calculate_delta_zero_window() {
        let delta = calculate_delta(80000.0, 0.0);
        assert_eq!(delta, 0.0);
    }

    #[test]
    fn test_calculate_fair_prob() {
        // Zero delta = 0.5 fair probability
        let prob = calculate_fair_prob(0.0);
        assert!((prob - 0.5).abs() < 0.001);

        // Large positive delta → approaches 0.95
        let prob = calculate_fair_prob(0.1);
        assert!(prob > 0.8);

        // Large negative delta → approaches 0.05
        let prob = calculate_fair_prob(-0.1);
        assert!(prob < 0.2);
    }

    #[test]
    fn test_check_price_limits() {
        let params = StrategyParams::default();
        assert!(check_price_limits(0.50, &params));
        assert!(!check_price_limits(0.20, &params));
        assert!(!check_price_limits(0.80, &params));
    }

    #[test]
    fn test_check_time_remaining() {
        let params = StrategyParams::default(); // min=5s, max=300s
        assert!(check_time_remaining(120, &params));   // 2 minutes - valid
        assert!(!check_time_remaining(3, &params));   // too short
        assert!(!check_time_remaining(400, &params)); // too long
    }

    #[test]
    fn test_check_delta() {
        let params = StrategyParams::default(); // min_delta = 0.02

        assert!(check_delta(0.05, &params, None)); // |0.05| > 0.02 ✓
        assert!(!check_delta(0.01, &params, None)); // |0.01| < 0.02 ✗

        assert!(check_delta(0.05, &params, Some("up"))); // 0.05 > 0.02 ✓
        assert!(!check_delta(0.01, &params, Some("up"))); // 0.01 < 0.02 ✗

        assert!(check_delta(-0.05, &params, Some("down"))); // -0.05 < -0.02 ✓
        assert!(!check_delta(-0.01, &params, Some("down"))); // -0.01 > -0.02 ✗
    }

    #[test]
    fn test_strategy_params_default() {
        let params = StrategyParams::default();
        assert_eq!(params.min_delta, 0.02);
        assert_eq!(params.max_delta, 5.0);
        assert_eq!(params.min_price, 0.25);
        assert_eq!(params.max_price, 0.75);
        assert!(params.min_odds.is_none());
    }

    #[test]
    fn test_has_sufficient_edge() {
        // Edge = |our_prob - market_prob|
        assert!(has_sufficient_edge(0.60, 0.50, 0.07));  // 10% edge >= 7%
        assert!(!has_sufficient_edge(0.55, 0.50, 0.07)); // 5% edge < 7%
        assert!(has_sufficient_edge(0.40, 0.50, 0.07));  // negative edge, abs = 10% >= 7%
        assert!(!has_sufficient_edge(0.50, 0.50, 0.07)); // no edge
    }
}
