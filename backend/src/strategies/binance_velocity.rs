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
                min_delta: 0.001,
                min_price: 0.20,
                max_price: 0.80,
                ..Default::default()
            },
        }
    }
}

impl Strategy for BinanceVelocityStrategy {
    fn name(&self) -> &str { "Binance Velocity" }
    fn description(&self) -> &str { "Analyzes Binance price velocity for early movement detection" }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        // polymarket_price mező használata (ez létezik a base.rs-ben)
        let pm_price = ctx.polymarket_price
            .or(if ctx.yes_price > 0.0 { Some(ctx.yes_price) } else { None })
            .unwrap_or(0.5);

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("Price outside range");
        }

        // FIX: btc_price_change a helyes mezőnév (nem btc_change!)
        let btc_change = match ctx.btc_price_change {
            Some(c) => c,
            None => match ctx.btc_velocity {
                Some(v) => v,
                None => return StrategyDecision::hold("No BTC velocity data"),
            },
        };

        let velocity_pct = btc_change.abs() * 100.0;

        if velocity_pct > self.params.min_delta {
            let confidence = (0.55 + velocity_pct * 2.0).min(0.82);
            if btc_change > 0.0 {
                return StrategyDecision::trade(
                    Signal::Yes,
                    confidence,
                    &format!("Binance velocity UP: {:.4}%", velocity_pct),
                );
            } else {
                return StrategyDecision::trade(
                    Signal::No,
                    confidence,
                    &format!("Binance velocity DOWN: {:.4}%", velocity_pct),
                );
            }
        }

        // Másodlagos: window delta
        // FIX: ctx.btc_price Option<f64>, ezért unwrap_or kell
        if let (Some(btc_price), Some(window_open)) = (ctx.btc_price, ctx.btc_window_open) {
            if window_open > 0.0 {
                let delta_pct = ((btc_price - window_open) / window_open) * 100.0;
                if delta_pct.abs() > self.params.min_delta {
                    if delta_pct > 0.0 {
                        return StrategyDecision::trade(Signal::Yes, 0.62,
                            &format!("Binance window delta UP: {:.4}%", delta_pct));
                    } else {
                        return StrategyDecision::trade(Signal::No, 0.62,
                            &format!("Binance window delta DOWN: {:.4}%", delta_pct));
                    }
                }
            }
        }

        StrategyDecision::hold(&format!("No Binance signal (velocity={:.4}%)", velocity_pct))
    }
}
