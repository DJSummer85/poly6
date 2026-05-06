//! Fair Value Strategy - Bets when Polymarket price deviates from fair probability
//!
//! Uses calculated fair probability (based on BTC) vs actual market price

use super::base::{
    calculate_fair_prob, check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct FairValueStrategy {
    params: StrategyParams,
}

impl FairValueStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for FairValueStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.03, // Min deviation from 0.5
                min_price: 0.25,
                max_price: 0.75,
                ..Default::default()
            },
        }
    }
}

impl Strategy for FairValueStrategy {
    fn name(&self) -> &str {
        "Fair Value"
    }

    fn description(&self) -> &str {
        "Bets when Polymarket price deviates significantly from fair probability"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        // Check time remaining
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        // Need polymarket price
        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No Polymarket price"),
        };

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("Price outside range");
        }

        // Calculate fair probability from BTC delta
        let btc_price = ctx.btc_price.unwrap_or(0.0);
        let window_open = ctx.btc_window_open.unwrap_or(btc_price);
        let delta_pct = ((btc_price - window_open) / window_open) * 100.0;

        let fair_prob = calculate_fair_prob(delta_pct);
        let deviation = (fair_prob - pm_price).abs();

        // Trade if deviation is significant
        if deviation > self.params.min_delta {
            if fair_prob > pm_price {
                // Market undervalues YES
                let confidence = (0.50 + deviation * 5.0).min(0.80);
                return StrategyDecision::trade(
                    Signal::Yes,
                    confidence,
                    &format!("Fair value: YES undervalued ({:.1}% vs {:.1}%)", fair_prob * 100.0, pm_price * 100.0),
                );
            } else {
                // Market overvalues YES - bet NO
                let confidence = (0.50 + deviation * 5.0).min(0.80);
                return StrategyDecision::trade(
                    Signal::No,
                    confidence,
                    &format!("Fair value: NO undervalued ({:.1}% vs {:.1}%)", (1.0 - fair_prob) * 100.0, (1.0 - pm_price) * 100.0),
                );
            }
        }

        StrategyDecision::hold("No significant deviation from fair value")
    }
}
