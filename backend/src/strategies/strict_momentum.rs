//! Strict Momentum Strategy - Only trades on VERY strong BTC moves
//!
//! Key insight: Small momentum is just noise. Only trade when BTC moves >0.15%
//! This significantly reduces false signals and improves win rate
//!
//! Why it works: At 50c odds you need ~67% win rate to break even.
//! By only trading on strong momentum, we filter out the noise trades.

use super::base::{
    check_time_remaining, calculate_delta,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct StrictMomentumStrategy {
    params: StrategyParams,
}

impl StrictMomentumStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for StrictMomentumStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.15,     // MUCH higher threshold - only strong moves
                min_price: 0.35,     // Don't trade extreme odds (safer)
                max_price: 0.65,
                min_time_remaining: 20000,  // Don't trade in last 20 seconds
                max_time_remaining: 280000,
                ..Default::default()
            },
        }
    }
}

impl Strategy for StrictMomentumStrategy {
    fn name(&self) -> &str {
        "Strict Momentum"
    }

    fn description(&self) -> &str {
        "Only trades on strong BTC momentum (>0.15%), filters out noise"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Time outside safe range");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No PM price"),
        };

        // Stricter price limits - avoid extreme odds
        if pm_price < self.params.min_price || pm_price > self.params.max_price {
            return StrategyDecision::hold(&format!(
                "PM price {:.1}% outside [{:.0}%-{:.0}%]", 
                pm_price * 100.0, 
                self.params.min_price * 100.0, 
                self.params.max_price * 100.0
            ));
        }

        // Calculate BTC delta from window
        let delta_pct = match (ctx.btc_price, ctx.btc_window_open) {
            (Some(cur), Some(open)) if open > 0.0 => calculate_delta(cur, open),
            _ => return StrategyDecision::hold("No BTC window data"),
        };

        let abs_delta = delta_pct.abs();

        // STRICT threshold - this is the key to filter noise
        if abs_delta < self.params.min_delta {
            return StrategyDecision::hold(&format!(
                "Delta {:.3}% < {:.2}% threshold (noise)", 
                delta_pct, self.params.min_delta
            ));
        }

        // Strong move detected - determine direction and confidence
        if delta_pct > 0.0 {
            // BTC going up
            let confidence = (0.65 + abs_delta * 1.5).min(0.88);
            return StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!("STRONG UP: BTC +{:.3}% (>{}%)", delta_pct, self.params.min_delta),
            );
        } else {
            // BTC going down
            let confidence = (0.65 + abs_delta * 1.5).min(0.88);
            return StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!("STRONG DOWN: BTC {:.3}% (<-{}%)", delta_pct, self.params.min_delta),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strict_ctx(btc_price: f64, window_open: f64, pm_price: f64, time: i64) -> StrategyContext {
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
    fn test_holds_on_small_move() {
        let strat = StrictMomentumStrategy::default();
        // Only 0.05% move - below 0.15% threshold
        let ctx = strict_ctx(80040.0, 80000.0, 0.50, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_trades_on_strong_up() {
        let strat = StrictMomentumStrategy::default();
        // 0.2% move - above threshold
        let ctx = strict_ctx(80160.0, 80000.0, 0.50, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Yes));
    }

    #[test]
    fn test_trades_on_strong_down() {
        let strat = StrictMomentumStrategy::default();
        // -0.2% move
        let ctx = strict_ctx(79840.0, 80000.0, 0.50, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::No));
    }

    #[test]
    fn test_rejects_extreme_odds() {
        let strat = StrictMomentumStrategy::default();
        // 0.2% move but extreme odds (80%) - should hold
        let ctx = strict_ctx(80160.0, 80000.0, 0.80, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }
}
