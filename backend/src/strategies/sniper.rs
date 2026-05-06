//! Sniper Strategy - Waits for perfect setup then executes
//!
//! High precision, low frequency strategy

use super::base::{
    check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct SniperStrategy {
    params: StrategyParams,
}

impl SniperStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for SniperStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.06,
                min_price: 0.35,
                max_price: 0.65,
                min_time_remaining: 60000, // At least 1 minute
                ..Default::default()
            },
        }
    }
}

impl Strategy for SniperStrategy {
    fn name(&self) -> &str {
        "Sniper"
    }

    fn description(&self) -> &str {
        "High precision strategy - waits for perfect setup"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        // Sniper needs specific conditions
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Not enough time remaining");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No PM price"),
        };

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("PM price not in sniper range");
        }

        // Need BTC price change
        let btc_change = match ctx.btc_price_change {
            Some(c) => c,
            None => return StrategyDecision::hold("No BTC data"),
        };

        let change_pct = btc_change.abs() * 100.0;

        // Strong move required for sniper
        if change_pct > self.params.min_delta {
            if btc_change > 0.0 {
                return StrategyDecision::trade(
                    Signal::Yes,
                    0.80,
                    &format!("Sniper: Perfect UP setup {:.2}%", change_pct),
                );
            } else {
                return StrategyDecision::trade(
                    Signal::No,
                    0.80,
                    &format!("Sniper: Perfect DOWN setup {:.2}%", change_pct),
                );
            }
        }

        StrategyDecision::hold("Waiting for sniper setup")
    }
}
