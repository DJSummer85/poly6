//! Mean Reversion Strategy - Bets when price is far from 0.5 (center)
//!
//! Expects price to return to center over time

use super::base::{
    check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct MeanReversionStrategy {
    params: StrategyParams,
}

impl MeanReversionStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for MeanReversionStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.15, // Need significant deviation from 0.5
                min_price: 0.20,
                max_price: 0.80,
                ..Default::default()
            },
        }
    }
}

impl Strategy for MeanReversionStrategy {
    fn name(&self) -> &str {
        "Mean Reversion"
    }

    fn description(&self) -> &str {
        "Bets when price is far from 0.5, expecting reversion to mean"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No Polymarket price"),
        };

        // Distance from center (0.5)
        let distance_from_center = (pm_price - 0.5).abs();

        if distance_from_center > self.params.min_delta {
            // Price far from center - bet on reversion
            if pm_price > 0.5 {
                // Price high - bet it goes down (NO)
                let confidence = (0.50 + distance_from_center * 3.0).min(0.78);
                return StrategyDecision::trade(
                    Signal::No,
                    confidence,
                    &format!("Mean reversion: price {:.1}% > 50%, expect down", pm_price * 100.0),
                );
            } else {
                // Price low - bet it goes up (YES)
                let confidence = (0.50 + distance_from_center * 3.0).min(0.78);
                return StrategyDecision::trade(
                    Signal::Yes,
                    confidence,
                    &format!("Mean reversion: price {:.1}% < 50%, expect up", pm_price * 100.0),
                );
            }
        }

        StrategyDecision::hold("Price too close to center for mean reversion")
    }
}
