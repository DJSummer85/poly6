//! Trend Strategy - Follows established trends
//!
//! Similar to momentum but requires stronger confirmation

use super::base::{
    calculate_delta, check_delta, check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct TrendStrategy {
    params: StrategyParams,
}

impl TrendStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for TrendStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.04, // Higher threshold than momentum
                min_price: 0.30,
                max_price: 0.70,
                ..Default::default()
            },
        }
    }
}

impl Strategy for TrendStrategy {
    fn name(&self) -> &str {
        "Trend"
    }

    fn description(&self) -> &str {
        "Follows established BTC trends with higher confidence"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        if let Some(pm_price) = ctx.polymarket_price {
            if !check_price_limits(pm_price, &self.params) {
                return StrategyDecision::hold("Price outside range");
            }
        }

        let btc_price = ctx.btc_price.unwrap_or(0.0);
        let window_open = ctx.btc_window_open.unwrap_or(btc_price);
        let delta_pct = calculate_delta(btc_price, window_open);

        // Strong trend required
        if check_delta(delta_pct, &self.params, Some("up")) {
            let confidence = (0.55 + delta_pct * 3.5).min(0.82);
            return StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!("Strong UP trend: +{:.2}%", delta_pct),
            );
        }

        if check_delta(delta_pct, &self.params, Some("down")) {
            let confidence = (0.55 + (-delta_pct) * 3.5).min(0.82);
            return StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!("Strong DOWN trend: {:.2}%", delta_pct),
            );
        }

        StrategyDecision::hold("No strong trend detected")
    }
}
