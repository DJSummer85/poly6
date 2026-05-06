//! Binance Velocity Strategy - Uses Binance-specific signals
//!
//! Analyzes Binance price velocity for early movement detection

use super::base::{
    check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct BinanceVelocityStrategy {
    params: StrategyParams,
}

impl BinanceVelocityStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for BinanceVelocityStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.025, // Velocity threshold
                min_price: 0.28,
                max_price: 0.72,
                ..Default::default()
            },
        }
    }
}

impl Strategy for BinanceVelocityStrategy {
    fn name(&self) -> &str {
        "Binance Velocity"
    }

    fn description(&self) -> &str {
        "Analyzes Binance price velocity for early movement detection"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No PM price"),
        };

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("Price outside range");
        }

        // Use BTC price change as velocity indicator
        let btc_change = match ctx.btc_price_change {
            Some(c) => c,
            None => return StrategyDecision::hold("No BTC velocity data"),
        };

        let velocity = btc_change.abs() * 100.0;

        // Check velocity threshold
        if velocity > self.params.min_delta {
            if btc_change > 0.0 {
                return StrategyDecision::trade(
                    Signal::Yes,
                    (0.55 + velocity * 4.0).min(0.76),
                    &format!("Binance velocity UP: {:.3}%", velocity),
                );
            } else {
                return StrategyDecision::trade(
                    Signal::No,
                    (0.55 + velocity * 4.0).min(0.76),
                    &format!("Binance velocity DOWN: {:.3}%", velocity),
                );
            }
        }

        // Also check window delta as secondary
        if let (Some(btc_price), Some(window_open)) = (ctx.btc_price, ctx.btc_window_open) {
            let delta_pct = ((btc_price - window_open) / window_open) * 100.0;
            if delta_pct.abs() > self.params.min_delta {
                if delta_pct > 0.0 {
                    return StrategyDecision::trade(
                        Signal::Yes,
                        0.65,
                        &format!("Binance window UP: {:.2}%", delta_pct),
                    );
                } else {
                    return StrategyDecision::trade(
                        Signal::No,
                        0.65,
                        &format!("Binance window DOWN: {:.2}%", delta_pct),
                    );
                }
            }
        }

        StrategyDecision::hold("No Binance velocity signal")
    }
}
