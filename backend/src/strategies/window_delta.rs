//! Window Delta Strategy - Compares BTC price change over time window
//!
//! Uses the price difference from window open to determine direction

use super::base::{
    calculate_delta, check_delta, check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct WindowDeltaStrategy {
    params: StrategyParams,
}

impl WindowDeltaStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for WindowDeltaStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.05,
                min_price: 0.30,
                max_price: 0.70,
                ..Default::default()
            },
        }
    }
}

impl Strategy for WindowDeltaStrategy {
    fn name(&self) -> &str {
        "Window Delta"
    }

    fn description(&self) -> &str {
        "Compares BTC price change from window open to predict market direction"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        // Check time remaining
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        // Check polymarket price limits
        if let Some(pm_price) = ctx.polymarket_price {
            if !check_price_limits(pm_price, &self.params) {
                return StrategyDecision::hold("Price outside acceptable range");
            }
        }

        // Calculate delta from window open
        let btc_price = ctx.btc_price.unwrap_or(0.0);
        let window_open = ctx.btc_window_open.unwrap_or(btc_price);
        let delta_pct = calculate_delta(btc_price, window_open);

        if check_delta(delta_pct, &self.params, Some("up")) {
            let confidence = (0.50 + delta_pct * 4.0).min(0.75);
            return StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!("BTC UP +{:.2}% from window open", delta_pct),
            );
        }

        if check_delta(delta_pct, &self.params, Some("down")) {
            let confidence = (0.50 + (-delta_pct) * 4.0).min(0.75);
            return StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!("BTC DOWN {:.2}% from window open", delta_pct),
            );
        }

        StrategyDecision::hold("Delta below threshold")
    }
}
