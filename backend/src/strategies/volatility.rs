//! Volatility Breakout Strategy - Catches sudden price movements
//!
//! Executes on significant short-term volatility spikes

use super::base::{
    check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct VolatilityStrategy {
    params: StrategyParams,
}

impl VolatilityStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for VolatilityStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.08, // High threshold for volatility
                min_price: 0.25,
                max_price: 0.75,
                ..Default::default()
            },
        }
    }
}

impl Strategy for VolatilityStrategy {
    fn name(&self) -> &str {
        "Volatility Breakout"
    }

    fn description(&self) -> &str {
        "Catches sudden volatility breakouts for quick trades"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        // Use price change from context for volatility detection
        let btc_change = match ctx.btc_price_change {
            Some(c) => c,
            None => return StrategyDecision::hold("No BTC price change data"),
        };

        let change_pct = btc_change.abs() * 100.0;

        // Check for volatility breakout
        if change_pct > self.params.min_delta {
            // Check polymarket price is reasonable
            if let Some(pm_price) = ctx.polymarket_price {
                if !check_price_limits(pm_price, &self.params) {
                    return StrategyDecision::hold("PM price outside range");
                }
            }

            if btc_change > 0.0 {
                let confidence = (0.60 + change_pct * 2.0).min(0.85);
                return StrategyDecision::trade(
                    Signal::Yes,
                    confidence,
                    &format!("Volatility breakout UP: {:.2}%", change_pct),
                );
            } else {
                let confidence = (0.60 + change_pct * 2.0).min(0.85);
                return StrategyDecision::trade(
                    Signal::No,
                    confidence,
                    &format!("Volatility breakout DOWN: {:.2}%", change_pct),
                );
            }
        }

        StrategyDecision::hold("No volatility breakout")
    }
}
