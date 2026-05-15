//! Momentum Strategy - Uses BTC price momentum to generate signals
//!
//! Based on short-term BTC price changes to predict market direction
//! 
//! DEPRECATED: Use StrategyExecutor::evaluate_momentum() in bot_executor.rs instead

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
                min_delta: 0.001,  // FIX: 0.1% (volt: 0.02, unit mismatch)
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
        "BTC momentum based trading - follows short-term price momentum (DEPRECATED)"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        // Check time remaining
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        // Check BTC price change from context
        if let Some(btc_change) = ctx.btc_price_change {
            // FIX: Unit consistency - compare in decimal form
            if btc_change.abs() > 0.0005 {
                if btc_change.abs() > self.params.min_delta {
                    let pct = btc_change * 100.0;
                    
                    // FIX: Higher confidence baseline (60% instead of 50%)
                    let confidence = if btc_change > 0.0 {
                        (0.60 + btc_change * 100.0).min(0.85)
                    } else {
                        (0.60 + (-btc_change) * 100.0).min(0.85)
                    };
                    
                    // FIX: Only trade if confidence >= 60%
                    if confidence < 0.60 {
                        return StrategyDecision::hold("Confidence too low");
                    }

                    if btc_change > 0.0 {
                        return StrategyDecision::trade(
                            Signal::Yes,
                            confidence,
                            &format!("BTC momentum +{:.3}%", pct),
                        );
                    } else {
                        return StrategyDecision::trade(
                            Signal::No,
                            confidence,
                            &format!("BTC momentum {:.3}%", pct),
                        );
                    }
                }
            }
        }

        // Fallback: use window delta
        if let (Some(btc_price), Some(window_open)) = (ctx.btc_price, ctx.btc_window_open) {
            let delta_pct = calculate_delta(btc_price, window_open);

            if check_delta(delta_pct, &self.params, Some("up")) {
                let confidence = (0.60 + delta_pct * 3.0).min(0.80);
                return StrategyDecision::trade(
                    Signal::Yes,
                    confidence,
                    &format!("Window momentum +{:.3}%", delta_pct),
                );
            }
            if check_delta(delta_pct, &self.params, Some("down")) {
                let confidence = (0.60 + (-delta_pct) * 3.0).min(0.80);
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
        ctx.btc_price_change = Some(0.03); // 3% increase
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Yes));
        assert!(decision.confidence > 0.60);
    }

    #[test]
    fn test_momentum_no_signal_small_change() {
        let strat = MomentumStrategy::default();
        let mut ctx = default_ctx();
        ctx.btc_price_change = Some(0.0001); // 0.01% — below threshold
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }
}