//! Confidence Calculator — 7-factor confidence model, Bayesian EV, and half-Kelly bet sizing
//!
//! Ported from polymarket-demo TypeScript project:
//! - calculate7FactorConfidence() → calculate_7_factor_confidence()
//! - calculateEV() with Bayesian update → calculate_bayesian_ev()
//! - calculateBetSize() with half-Kelly → calculate_half_kelly_bet()

use crate::trading::bot_executor::strategies::StrategyContext;

/// 7-factor confidence model
///
/// Each factor contributes to a unified, calibrated confidence score [0.0, 1.0]:
///
/// 1. **BTC delta** — price change since market open (window_open)
/// 2. **Tick change** — recent 30s BTC price change
/// 3. **Time remaining** — confidence improves with more time (less random noise)
/// 4. **Odds extremity** — extreme YES/NO prices signal market conviction
/// 5. **Volatility** — high volatility reduces confidence
/// 6. **Momentum (acceleration)** — accelerating price movement in our direction
/// 7. **Velocity** — sustained directional price movement
pub fn calculate_7_factor_confidence(ctx: &StrategyContext, side: &str) -> f64 {
    // Factor 1: BTC delta from market open
    let btc_delta = ctx
        .btc_window_open
        .map(|open| {
            if open > 0.0 {
                (ctx.btc_price - open) / open
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    // Factor 2: Tick change (30s window)
    let tick_change = ctx.btc_change.unwrap_or(0.0);

    // Factor 3: Time remaining — normalized to 300s (5 min)
    let time_ratio = (ctx.time_remaining as f64 / 300.0).clamp(0.0, 1.0);

    // Factor 5: Volatility — higher vol = less confidence (see odds check below) — higher vol = less confidence
    let volatility = ctx.btc_volatility.unwrap_or(0.001);
    let vol_penalty = if volatility > 0.005 {
        0.15 // High vol: -15% confidence
    } else if volatility > 0.003 {
        0.08 // Medium vol: -8%
    } else {
        0.02 // Low vol: -2%
    };

    // Factor 6: Momentum (acceleration in our direction)
    let acceleration = ctx.btc_acceleration.unwrap_or(0.0);
    let accel_in_direction = if side == "YES" {
        acceleration
    } else {
        -acceleration
    };
    let momentum_boost = (accel_in_direction * 800.0).clamp(-0.10, 0.15);

    // Factor 7: Velocity (sustained direction)
    let velocity = ctx.btc_velocity.unwrap_or(0.0);
    let vel_in_direction = if side == "YES" {
        velocity
    } else {
        -velocity
    };
    let velocity_boost = (vel_in_direction * 1200.0).clamp(-0.10, 0.20);

    // --- Combine factors ---

    // Base: start at 0.50 (neutral / uninformed)
    let mut confidence = 0.50;

    // Directional delta contribution (max ±25%)
    let delta_direction = if side == "YES" {
        btc_delta
    } else {
        -btc_delta
    };
    confidence += (delta_direction * 50.0).clamp(-0.25, 0.25);

    // Tick change contribution (max ±15%)
    let tick_direction = if side == "YES" {
        tick_change
    } else {
        -tick_change
    };
    confidence += (tick_direction * 80.0).clamp(-0.15, 0.15);

    // Time remaining: more time → slightly higher confidence (max +5%)
    confidence += (time_ratio * 0.05).min(0.05);

    // Odds extremity: if the market strongly disagrees with us, reduce confidence
    //   e.g. side=YES, but YES price is 0.30 → market disagrees → penalty
    let odds_disagreement = if side == "YES" {
        1.0 - ctx.yes_price
    } else {
        1.0 - ctx.no_price
    };
    confidence -= (odds_disagreement * 0.08).min(0.08); // Max -8% for disagreement

    // Volatility penalty
    confidence -= vol_penalty;

    // Momentum boost
    confidence += momentum_boost;

    // Velocity boost
    confidence += velocity_boost;

    // Clamp to calibrated range
    confidence.clamp(0.05, 0.95)
}

/// Bayesian Expected Value calculation
///
/// **Full formula:**
/// ```text
/// EV = P(win) × (1 - cost) × (1 - fee) - P(lose) × cost
/// ```
///
/// **Bayesian update** from signal strength:
/// ```text
/// posterior = (prior × likelihood) / (prior × likelihood + (1-prior) × (1-likelihood))
/// ```
///
/// Returns `(posterior_probability, expected_value)`.
pub fn calculate_bayesian_ev(
    raw_confidence: f64,
    cost: f64,
    fee_rate: f64,
    btc_signal_strength: f64,
) -> (f64, f64) {
    // --- Bayesian update ---
    let prior = 0.50; // Neutral prior for 5-min BTC up/down markets

    // Map signal strength → likelihood [0.50, 0.90]
    //   strength=0.0 → likelihood=0.50 (uninformative)
    //   strength=1.0 → likelihood=0.90 (strong signal)
    let signal_strength = btc_signal_strength.abs().clamp(0.0, 1.0);
    let likelihood = 0.50 + signal_strength * 0.40;

    let posterior = if signal_strength > 0.01 {
        let numerator = prior * likelihood;
        let denominator = numerator + (1.0 - prior) * (1.0 - likelihood);
        if denominator > 0.0 {
            numerator / denominator
        } else {
            raw_confidence
        }
    } else {
        raw_confidence
    };

    // Blend Bayesian posterior with raw confidence using signal strength as weight
    let blend_weight = signal_strength * 0.5; // Max 50% weight to Bayesian
    let final_prob = posterior * blend_weight + raw_confidence * (1.0 - blend_weight);

    // --- EV calculation ---
    //   If we win: we get (1 - cost) back, minus fee
    //   If we lose: we lose the cost
    let p_win = final_prob;
    let p_lose = 1.0 - p_win;

    let net_profit_if_win = (1.0 - cost) * (1.0 - fee_rate);
    let loss_if_lose = cost;

    let expected_value = (p_win * net_profit_if_win) - (p_lose * loss_if_lose);

    (final_prob, expected_value)
}

/// Calculate position size using **half-Kelly Criterion** with feedback loops.
///
/// ```text
/// f* = ((b × p - q) / b) × kelly_fraction
///
/// where:
///   b = (1 - price) / price   (odds multiplier)
///   p = confidence             (win probability)
///   q = 1 - p                  (loss probability)
/// ```
///
/// The result is then capped by `max_bet_fraction` of bankroll
/// and multiplied by the `risk_multiplier` (from consecutive loss tracking).
pub fn calculate_half_kelly_bet(
    bankroll: f64,
    confidence: f64,
    price: f64,
    kelly_fraction: f64,
    risk_multiplier: f64,
    max_bet_fraction: f64,
    min_bet: f64,
) -> f64 {
    // No edge: if confidence doesn't exceed the token price, skip
    if confidence <= price {
        return 0.0;
    }

    // Odds multiplier
    let b = if price > 0.0 && price < 1.0 {
        (1.0 - price) / price
    } else {
        1.0
    };

    let p = confidence;
    let q = 1.0 - p;

    let mut kelly = (b * p - q) / b;
    if kelly <= 0.0 {
        return 0.0;
    }

    // Apply fraction (half-Kelly = 0.50, quarter-Kelly = 0.25)
    kelly *= kelly_fraction;

    // Apply risk multiplier from loss tracking
    kelly *= risk_multiplier;

    // Cap at max fraction of bankroll
    kelly = kelly.min(max_bet_fraction);

    // Convert to dollar amount
    let bet_size = bankroll * kelly;

    // Enforce minimum
    bet_size.max(min_bet)
}

/// Decay factor for confidence based on consecutive losses.
///
/// Returns a multiplier [0.0, 1.0] to apply to confidence:
/// - 0 losses: 1.0 (no adjustment)
/// - 1 loss:   0.80
/// - 2 losses: 0.65
/// - 3 losses: 0.50
/// - 4 losses: 0.35
/// - 5+ losses: 0.20 (strong penalty)
pub fn confidence_decay_from_losses(consecutive_losses: u32) -> f64 {
    match consecutive_losses {
        0 => 1.0,
        1 => 0.80,
        2 => 0.65,
        3 => 0.50,
        4 => 0.35,
        _ => 0.20,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::bot_executor::strategies::StrategyContext;

    fn test_context() -> StrategyContext {
        StrategyContext {
            btc_price: 85000.0,
            btc_change: Some(0.002),
            btc_window_open: Some(84800.0),
            yes_price: 0.55,
            no_price: 0.45,
            time_remaining: 180,
            btc_velocity: Some(0.0001),
            btc_acceleration: Some(0.00001),
            btc_volatility: Some(0.001),
            market_start_price: Some(84800.0),
        }
    }

    #[test]
    fn test_seven_factor_bullish() {
        let ctx = test_context();
        let conf = calculate_7_factor_confidence(&ctx, "YES");
        assert!(
            conf > 0.50,
            "Expected YES confidence > 0.50 for bullish signal, got {:.4}",
            conf
        );
        assert!(conf < 1.0, "Confidence should be < 1.0");
    }

    #[test]
    fn test_seven_factor_bearish() {
        let ctx = test_context();
        let conf = calculate_7_factor_confidence(&ctx, "NO");
        assert!(
            conf < 0.50,
            "Expected NO confidence < 0.50 for bullish signal, got {:.4}",
            conf
        );
    }

    #[test]
    fn test_seven_factor_clamped() {
        let mut ctx = test_context();
        ctx.btc_change = Some(0.05); // Huge move → should clamp
        let conf = calculate_7_factor_confidence(&ctx, "YES");
        assert!(conf <= 0.95, "Should clamp at 0.95, got {:.4}", conf);

        ctx.btc_change = Some(-0.05);
        let conf = calculate_7_factor_confidence(&ctx, "NO");
        assert!(conf <= 0.95, "Should clamp at 0.95, got {:.4}", conf);
    }

    #[test]
    fn test_bayesian_ev_positive() {
        let (prob, ev) = calculate_bayesian_ev(0.70, 0.55, 0.02, 0.5);
        assert!(prob > 0.50);
        assert!(ev > 0.0, "Expected positive EV, got {:.6}", ev);
    }

    #[test]
    fn test_bayesian_ev_negative() {
        let (_, ev) = calculate_bayesian_ev(0.52, 0.55, 0.02, 0.1);
        assert!(ev < 0.0, "Expected negative EV, got {:.6}", ev);
    }

    #[test]
    fn test_bayesian_ev_with_fees() {
        // Even with high confidence, fees can make EV negative
        let (_, ev) = calculate_bayesian_ev(0.60, 0.58, 0.02, 0.3);
        assert!(
            ev < 0.02,
            "EV should be small or negative with high fees + tight edge"
        );
    }

    #[test]
    fn test_half_kelly_normal() {
        let bet = calculate_half_kelly_bet(100.0, 0.70, 0.55, 0.5, 1.0, 0.25, 0.1);
        assert!(bet > 0.0, "Expected positive bet");
        assert!(bet <= 25.0, "Bet should not exceed 25% of bankroll, got {:.2}", bet);
    }

    #[test]
    fn test_half_kelly_no_edge() {
        let bet = calculate_half_kelly_bet(100.0, 0.50, 0.55, 0.5, 1.0, 0.25, 0.1);
        assert_eq!(bet, 0.0, "Expected 0 bet for no edge");
    }

    #[test]
    fn test_half_kelly_risk_multiplier() {
        let full_bet = calculate_half_kelly_bet(100.0, 0.70, 0.55, 0.5, 1.0, 0.25, 0.1);
        let reduced_bet = calculate_half_kelly_bet(100.0, 0.70, 0.55, 0.5, 0.25, 0.25, 0.1);
        assert!(
            reduced_bet <= full_bet,
            "Risk multiplier should reduce bet size"
        );
    }

    #[test]
    fn test_confidence_decay() {
        assert_eq!(confidence_decay_from_losses(0), 1.0);
        assert_eq!(confidence_decay_from_losses(1), 0.80);
        assert_eq!(confidence_decay_from_losses(5), 0.20);
    }
}
