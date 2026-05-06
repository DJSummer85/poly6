//! Contrarian Strategy - Bets against extreme price movements
//!
//! Opposite of momentum: buys when price moved down, sells when moved up

use super::base::{
    calculate_delta, check_delta, check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct ContrarianStrategy {
    params: StrategyParams,
}

impl ContrarianStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for ContrarianStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.03,
                min_price: 0.25,
                max_price: 0.75,
                ..Default::default()
            },
        }
    }
}

impl Strategy for ContrarianStrategy {
    fn name(&self) -> &str {
        "Contrarian"
    }

    fn description(&self) -> &str {
        "Bets against extreme price movements - buys dips, sells rallies"
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

        // Calculate delta
        let btc_price = ctx.btc_price.unwrap_or(0.0);
        let window_open = ctx.btc_window_open.unwrap_or(btc_price);
        let delta_pct = calculate_delta(btc_price, window_open);

        // Contrarian: bet against the move
        if check_delta(delta_pct, &self.params, Some("up")) {
            // Price went up significantly - bet NO (it will reverse)
            let confidence = (0.50 + delta_pct * 3.0).min(0.72);
            return StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!("Contrarian: betting against UP {:.2}%", delta_pct),
            );
        }

        if check_delta(delta_pct, &self.params, Some("down")) {
            // Price went down significantly - bet YES (it will reverse)
            let confidence = (0.50 + (-delta_pct) * 3.0).min(0.72);
            return StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!("Contrarian: betting against DOWN {:.2}%", delta_pct),
            );
        }

        StrategyDecision::hold("No extreme movement to bet against")
    }
}
