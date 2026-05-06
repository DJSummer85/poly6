//! Momentum Strategy - Uses BTC price momentum to generate signals
//!
//! Based on short-term BTC price changes to predict market direction

use super::base::{
    check_delta, check_time_remaining, calculate_delta, Signal, Strategy, StrategyContext,
    StrategyDecision, StrategyParams,
};

pub struct MomentumStrategy {
    params: StrategyParams,
}

impl MomentumStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for MomentumStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.02,
                ..Default::default()
            },
        }
    }
}

impl Strategy for MomentumStrategy {
    fn name(&self) -> &str {
        "Momentum"
    }

    fn description(&self) -> &str {
        "BTC momentum based trading - follows short-term price momentum"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        // Check time remaining
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        // Check BTC price change from context
        if let Some(btc_change) = ctx.btc_price_change {
            if btc_change.abs() > 0.0005 {
                let pct = btc_change * 100.0;

                if pct > self.params.min_delta {
                    let confidence = (0.50 + pct * 5.0).min(0.78);
                    return StrategyDecision::trade(
                        Signal::Yes,
                        confidence,
                        &format!("BTC momentum +{:.3}%", pct),
                    );
                }
                if pct < -self.params.min_delta {
                    let confidence = (0.50 + (-pct) * 5.0).min(0.78);
                    return StrategyDecision::trade(
                        Signal::No,
                        confidence,
                        &format!("BTC momentum {:.3}%", pct),
                    );
                }
            }
        }

        // Fallback: use window delta
        if let (Some(btc_price), Some(window_open)) = (ctx.btc_price, ctx.btc_window_open) {
            let delta_pct = calculate_delta(btc_price, window_open);

            if check_delta(delta_pct, &self.params, Some("up")) {
                let confidence = (0.50 + delta_pct * 4.0).min(0.70);
                return StrategyDecision::trade(
                    Signal::Yes,
                    confidence,
                    &format!("Window momentum +{:.3}%", delta_pct),
                );
            }
            if check_delta(delta_pct, &self.params, Some("down")) {
                let confidence = (0.50 + (-delta_pct) * 4.0).min(0.70);
                return StrategyDecision::trade(
                    Signal::No,
                    confidence,
                    &format!("Window momentum {:.3}%", delta_pct),
                );
            }
        }

        StrategyDecision::hold("No significant momentum detected")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ctx() -> StrategyContext {
        StrategyContext {
            btc_price: Some(80000.0),
            btc_price_change: None,
            btc_window_open: Some(80000.0),
            polymarket_price: Some(0.50),
            time_remaining: 120000,
            order_book_spread: None,
        }
    }

    #[test]
    fn test_momentum_name() {
        let strat = MomentumStrategy::default();
        assert_eq!(strat.name(), "Momentum");
    }

    #[test]
    fn test_momentum_hold_no_change() {
        let strat = MomentumStrategy::default();
        let ctx = default_ctx();
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_momentum_yes_signal() {
        let strat = MomentumStrategy::default();
        let mut ctx = default_ctx();
        ctx.btc_price_change = Some(0.003); // 0.3% increase
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Yes));
        assert!(decision.confidence > 0.5);
    }

    #[test]
    fn test_momentum_no_signal_small_change() {
        let strat = MomentumStrategy::default();
        let mut ctx = default_ctx();
        ctx.btc_price_change = Some(0.0001); // 0.01% — below threshold
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_momentum_hold_near_close() {
        let strat = MomentumStrategy::default();
        let mut ctx = default_ctx();
        ctx.time_remaining = 5000; // Too close to close
        ctx.btc_price_change = Some(0.01);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_momentum_window_delta_fallback() {
        let strat = MomentumStrategy::default();
        let mut ctx = default_ctx();
        ctx.btc_price = Some(81000.0); // 1.25% above window open
        ctx.btc_window_open = Some(80000.0);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Yes));
    }
}
