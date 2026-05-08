//! Edge Hunter Strategy - Only trades when we have a mathematical edge
//!
//! Key insight: Trade when OUR calculated probability > MARKET implied probability
//! This ensures positive expected value over time
//!
//! Example: If BTC moved +0.15% and Polymarket shows 52%, we might calculate 54%
//! We have edge if 54% > 52% (by margin of at least 3%)

use super::base::{
    check_price_limits, check_time_remaining, calculate_delta, calculate_fair_prob,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct EdgeHunterStrategy {
    params: StrategyParams,
    min_edge: f64,  // Minimum edge percentage to trigger trade
}

impl EdgeHunterStrategy {
    pub fn new(params: StrategyParams, min_edge: f64) -> Self {
        Self { params, min_edge }
    }
}

impl Default for EdgeHunterStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.05,      // Need significant BTC move
                min_price: 0.35,      // Don't trade extreme odds
                max_price: 0.65,
                min_time_remaining: 15000,
                max_time_remaining: 270000,
                ..Default::default()
            },
            min_edge: 0.03,          // Need 3% edge minimum
        }
    }
}

impl Strategy for EdgeHunterStrategy {
    fn name(&self) -> &str {
        "Edge Hunter"
    }

    fn description(&self) -> &str {
        "Only trades when our probability > market probability by margin"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Time outside range");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No PM price"),
        };

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("PM price outside range");
        }

        // Calculate BTC delta
        let (delta_pct, _btc_price) = match (ctx.btc_price, ctx.btc_window_open) {
            (Some(cur), Some(open)) if open > 0.0 => {
                (calculate_delta(cur, open), cur)
            }
            _ => return StrategyDecision::hold("No BTC window data"),
        };

        // Need minimum BTC movement
        if delta_pct.abs() < self.params.min_delta {
            return StrategyDecision::hold(&format!(
                "BTC delta {:.3}% < threshold {:.2}%", 
                delta_pct, self.params.min_delta
            ));
        }

        // Calculate our fair probability from delta
        let our_prob = calculate_fair_prob(delta_pct / 100.0);

        // Get market implied probability (YES price = probability)
        let market_prob = pm_price;

        // Calculate edge: positive means we think it's more likely than market
        let edge = our_prob - market_prob;

        // Only trade if we have sufficient edge
        if edge > self.min_edge {
            // Our probability > market - market is undervaluing our outcome
            let confidence = (0.55 + edge * 3.0).min(0.82);
            return StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!(
                    "EDGE +{:.1}%: our {:.1}% > market {:.1}% (BTC {:.2}%)",
                    edge * 100.0, our_prob * 100.0, market_prob * 100.0, delta_pct
                ),
            );
        }

        if edge < -self.min_edge {
            // Market thinks this is MORE likely than we do - bet against
            let confidence = (0.55 + (-edge) * 3.0).min(0.82);
            return StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!(
                    "EDGE +{:.1}%: our {:.1}% < market {:.1}% (BTC {:.2}%)",
                    (-edge) * 100.0, our_prob * 100.0, market_prob * 100.0, delta_pct
                ),
            );
        }

        StrategyDecision::hold(&format!(
            "No edge: our {:.1}% vs market {:.1}% (need {:.1}% edge)",
            our_prob * 100.0, market_prob * 100.0, self.min_edge * 100.0
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge_ctx(btc_price: f64, window_open: f64, pm_price: f64, time: i64) -> StrategyContext {
        StrategyContext {
            btc_price: Some(btc_price),
            btc_price_change: None,
            btc_window_open: Some(window_open),
            polymarket_price: Some(pm_price),
            time_remaining: time,
            order_book_spread: None,
        }
    }

    #[test]
    fn test_positive_edge_trades_yes() {
        let strat = EdgeHunterStrategy::default();
        // BTC up 0.1%, PM at 52%, our calc says 54% -> edge = +2%
        // Need 3% edge minimum, so this won't trade
        let ctx = edge_ctx(80080.0, 80000.0, 0.52, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_strong_positive_edge_trades() {
        let strat = EdgeHunterStrategy::default();
        // BTC up 0.2%, PM at 50%, our calc says 58% -> edge = +8%
        let ctx = edge_ctx(80160.0, 80000.0, 0.50, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Yes));
    }
}
