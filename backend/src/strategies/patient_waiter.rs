//! Patient Waiter Strategy - Waits for PERFECT setups only
//!
//! Key insight: Don't trade just to trade. Wait for conditions where:
//! 1. Odds are near 50% (maximum expected value zone)
//! 2. BTC has moved enough to give us directional conviction
//! 3. Plenty of time remaining
//!
//! This strategy trades VERY infrequently but only when odds are in our favor
//! Expected to have high win rate but low number of trades

use super::base::{
    check_time_remaining, calculate_delta,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct PatientWaiterStrategy {
    params: StrategyParams,
    /// Only trade when odds are within this distance from 0.5
    odds_tolerance: f64,
}

impl PatientWaiterStrategy {
    pub fn new(params: StrategyParams, odds_tolerance: f64) -> Self {
        Self { params, odds_tolerance }
    }
}

impl Default for PatientWaiterStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.12,        // Moderate threshold
                min_price: 0.45,        // Only near 50% odds (45-55%)
                max_price: 0.55,
                min_time_remaining: 30000,  // Need at least 30 seconds
                max_time_remaining: 290000,
                ..Default::default()
            },
            odds_tolerance: 0.05,  // Within 5% of 50c
        }
    }
}

impl Strategy for PatientWaiterStrategy {
    fn name(&self) -> &str {
        "Patient Waiter"
    }

    fn description(&self) -> &str {
        "Waits for perfect setups near 50% odds, trades infrequently but precisely"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        // Time check first
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Time not in sweet spot");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No PM price"),
        };

        // KEY: Only trade when odds are near 50% (sweet spot for value)
        let distance_from_50 = (pm_price - 0.5).abs();
        if distance_from_50 > self.odds_tolerance {
            return StrategyDecision::hold(&format!(
                "Odds {:.1}% not in 45-55% sweet spot", pm_price * 100.0
            ));
        }

        // Calculate BTC delta
        let delta_pct = match (ctx.btc_price, ctx.btc_window_open) {
            (Some(cur), Some(open)) if open > 0.0 => calculate_delta(cur, open),
            _ => return StrategyDecision::hold("No BTC window data"),
        };

        // Need clear directional move
        if delta_pct.abs() < self.params.min_delta {
            return StrategyDecision::hold(&format!(
                "BTC delta {:.3}% < {:.2}% (no clear direction)", 
                delta_pct, self.params.min_delta
            ));
        }

        // Perfect setup: near 50c odds + clear BTC move
        if delta_pct > 0.0 {
            // BTC moving up, odds near 50% = good value on YES
            let confidence = (0.60 + delta_pct * 2.0).min(0.85);
            return StrategyDecision::trade(
                Signal::Yes,
                confidence,
                &format!(
                    "PERFECT: BTC +{:.3}%, YES@{:.1}% (sweet spot)",
                    delta_pct, pm_price * 100.0
                ),
            );
        } else {
            // BTC moving down, odds near 50% = good value on NO
            let confidence = (0.60 + (-delta_pct) * 2.0).min(0.85);
            return StrategyDecision::trade(
                Signal::No,
                confidence,
                &format!(
                    "PERFECT: BTC {:.3}%, NO@{:.1}% (sweet spot)",
                    delta_pct, (1.0 - pm_price) * 100.0
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patient_ctx(btc_price: f64, window_open: f64, pm_price: f64, time: i64) -> StrategyContext {
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
    fn test_holds_when_odds_not_sweet() {
        let strat = PatientWaiterStrategy::default();
        // 0.2% BTC move but odds at 70% - should hold
        let ctx = patient_ctx(80160.0, 80000.0, 0.70, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }

    #[test]
    fn test_trades_in_sweet_spot() {
        let strat = PatientWaiterStrategy::default();
        // 0.2% BTC move and odds at 52% (near 50%)
        let ctx = patient_ctx(80160.0, 80000.0, 0.52, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Yes));
    }

    #[test]
    fn test_holds_on_insufficient_move() {
        let strat = PatientWaiterStrategy::default();
        // Near 50% odds but only 0.05% BTC move
        let ctx = patient_ctx(80040.0, 80000.0, 0.51, 120000);
        let decision = strat.evaluate(&ctx);
        assert!(matches!(decision.signal, Signal::Hold));
    }
}
