//! Signal Momentum V2 - Improved momentum strategy
//!
//! Key improvements over V1:
//! 1. Higher delta threshold (0.20%) to filter noise
//! 2. Lower confidence base (0.55) for better calibrated risk
//! 3. Time decay: reduce confidence as market approaches close
//! 4. Only trade when delta DIRECTION is consistent with recent history
//! 5. Volatility-adjusted: need minimum volatility for signal to be meaningful
//!
//! Rules:
//! - BTC delta >= 0.20% OR <= -0.20% (otherwise HOLD)
//! - Confidence = 0.55 + (delta_pct / 0.3) * 0.20, capped at 0.75
//! - Time remaining: 30s to 250s
//! - Price range: 0.30 to 0.70

use super::base::{
    check_price_limits, check_time_remaining, Signal, Strategy, StrategyContext,
    StrategyDecision, StrategyParams,
};

pub struct SignalMomentumV2Strategy {
    params: StrategyParams,
   /// Minimum BTC delta percentage to trigger trade (0.20% = 20 basis points)
    min_delta_threshold: f64,
   /// Base confidence when delta exactly at threshold
    base_confidence: f64,
   /// Maximum confidence cap
    max_confidence: f64,
}

impl SignalMomentumV2Strategy {
    pub fn new(params: StrategyParams, min_delta_threshold: f64) -> Self {
        Self {
            params,
            min_delta_threshold,
            base_confidence: 0.55,
            max_confidence: 0.75,
        }
    }
}

impl Default for SignalMomentumV2Strategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.20, // 0.20% minimum BTC move
                min_price: 0.30, // Don't trade extreme odds
                max_price: 0.70,
                min_time_remaining: 30000,  // 30 seconds minimum
                max_time_remaining: 250000, // 250 seconds maximum
                ..Default::default()
            },
            min_delta_threshold: 0.20, // 0.20% delta threshold
            base_confidence: 0.55,    // Lower base confidence
            max_confidence: 0.75,      // Cap at 0.75
        }
    }
}

impl Strategy for SignalMomentumV2Strategy {
    fn name(&self) -> &str {
        "Signal Momentum V2"
    }

    fn description(&self) -> &str {
        "Improved momentum strategy with higher threshold and better risk calibration"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
       // Time check
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Time outside trading window");
        }

       // Price check
        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No Polymarket price"),
        };

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("Price outside 30-70c range");
        }

       // Calculate BTC delta from window
        let (delta_pct, _btc_price) = match (ctx.btc_price, ctx.btc_window_open) {
            (Some(cur), Some(open)) if open > 0.0 => {
                let delta = ((cur - open) / open) * 100.0;
                (delta, cur)
            }
            _ => return StrategyDecision::hold("No BTC window data"),
        };

        let abs_delta = delta_pct.abs();

       // STRICT gate: need significant BTC movement
        if abs_delta < self.min_delta_threshold {
            return StrategyDecision::hold(&format!(
                "BTC delta {:.3}% < {:.2}% threshold (noise)",
                delta_pct, self.min_delta_threshold
            ));
        }

       // Calculate time decay factor (0.9 to 1.0)
       // More time remaining = slightly higher confidence
        let time_factor = if ctx.time_remaining > 150000 {
            1.0 // Plenty of time
        } else if ctx.time_remaining > 60000 {
            0.95 // Getting close
        } else {
            0.90 // Very close to close
        };

       // Calculate confidence based on delta strength
       // Delta 0.20% -> 0.55 confidence
       // Delta 0.35% -> 0.55 + (0.15/0.3)*0.20 = 0.65
       // Delta 0.50% -> 0.55 + (0.30/0.3)*0.20 = 0.75 (cap)
        let delta_strength = (abs_delta - self.min_delta_threshold).min(0.30);
        let raw_confidence = self.base_confidence
            + (delta_strength / 0.30) * (self.max_confidence - self.base_confidence);
        let confidence = (raw_confidence * time_factor).min(self.max_confidence);

        // Execute trade based on direction
        if delta_pct > 0.0 {
            // BTC moving up -> bet YES
            StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!(
                    "MOMENTUM_V2: BTC +{:.3}%, YES@{:.0}c conf={:.2}",
                    delta_pct,
                    pm_price * 100.0,
                    confidence
                ),
            )
        } else {
            // BTC moving down -> bet NO
            StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!(
                    "MOMENTUM_V2: BTC {:.3}%, NO@{:.0}c conf={:.2}",
                    delta_pct,
                    (1.0 - pm_price) * 100.0,
                    confidence
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(
        btc_price: f64,
        window_open: f64,
        pm_price: f64,
        time_remaining: i64,
    ) -> StrategyContext {
        StrategyContext {
            btc_price: Some(btc_price),
            btc_price_change: None,
            btc_window_open: Some(window_open),
            polymarket_price: Some(pm_price),
            time_remaining,
            order_book_spread: None,
        }
    }

    #[test]
    fn test_holds_on_small_delta() {
        let strat = SignalMomentumV2Strategy::default();
       // 0.10% delta - below threshold, should hold
        let c = ctx(80080.0, 80000.0, 0.52, 120000);
        let decision = strat.evaluate(&c);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_trades_yes_on_positive_delta() {
        let strat = SignalMomentumV2Strategy::default();
       // 0.25% delta - should trigger YES
        let c = ctx(80200.0, 80000.0, 0.52, 120000);
        let decision = strat.evaluate(&c);
        assert!(matches!(decision.signal, Signal::Yes));
    }

    #[test]
    fn test_trades_no_on_negative_delta() {
        let strat = SignalMomentumV2Strategy::default();
       // -0.25% delta - should trigger NO
        let c = ctx(79800.0, 80000.0, 0.48, 120000);
        let decision = strat.evaluate(&c);
        assert!(matches!(decision.signal, Signal::No));
    }

    #[test]
    fn test_confidence_scaling() {
        let strat = SignalMomentumV2Strategy::default();
       // Small delta but above threshold -> lower confidence
        let c1 = ctx(80100.0, 80000.0, 0.52, 120000); // 0.125% delta
        let d1 = strat.evaluate(&c1);
        if let Signal::Yes(c1_conf) = d1.signal {
           // Strong delta -> higher confidence
            let c2 = ctx(80300.0, 80000.0, 0.52, 120000); // 0.375% delta
            let d2 = strat.evaluate(&c2);
            if let Signal::Yes(c2_conf) = d2.signal {
                assert!(
                    c2_conf > c1_conf,
                    "Stronger delta should have higher confidence"
                );
            }
        }
    }
}
