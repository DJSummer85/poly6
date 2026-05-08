//! Strategy execution for bot executor

use serde::{Deserialize, Serialize};

/// Market snapshot - complete market state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub market_id: String,
    pub question: String,
    pub yes_price: f64,
    pub no_price: f64,
    pub spread: f64,
    pub volume: f64,
    pub liquidity: f64,
    pub time_remaining: i64, // seconds
    pub btc_price: f64,
    pub btc_change_24h: Option<f64>,
    pub btc_velocity: Option<f64>,
    pub btc_acceleration: Option<f64>,
    pub btc_volatility: Option<f64>,
    pub btc_window_open: Option<f64>,
    pub order_book_bids: Vec<f64>,
    pub order_book_asks: Vec<f64>,
    pub fetched_at: i64, // unix timestamp ms
}

impl MarketSnapshot {
    pub fn new(market_id: String) -> Self {
        Self {
            market_id,
            question: String::new(),
            yes_price: 0.5,
            no_price: 0.5,
            spread: 0.0,
            volume: 0.0,
            liquidity: 0.0,
            time_remaining: 0,
            btc_price: 0.0,
            btc_change_24h: None,
            btc_velocity: None,
            btc_acceleration: None,
            btc_volatility: None,
            btc_window_open: None,
            order_book_bids: Vec::new(),
            order_book_asks: Vec::new(),
            fetched_at: 0,
        }
    }

    /// Build StrategyContext from this MarketSnapshot
    pub fn to_strategy_context(&self) -> StrategyContext {
        StrategyContext {
            btc_price: self.btc_price,
            btc_change: self.btc_change_24h,
            btc_window_open: self.btc_window_open,
            yes_price: self.yes_price,
            no_price: self.no_price,
            time_remaining: self.time_remaining * 1000, // convert to ms
            btc_velocity: self.btc_velocity,
            btc_acceleration: self.btc_acceleration,
            btc_volatility: self.btc_volatility,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    Yes(f64),  // Buy YES, confidence 0-1
    No(f64),   // Buy NO (sell YES), confidence 0-1
    Hold(String), // No action, reason
}

/// Strategy context - all data needed for strategy evaluation
#[derive(Debug, Clone)]
pub struct StrategyContext {
    pub btc_price: f64,
    pub btc_change: Option<f64>,
    pub btc_window_open: Option<f64>,
    pub yes_price: f64,
    pub no_price: f64,
    pub time_remaining: i64, // milliseconds
    // Velocity & acceleration (rate of change per second from Binance)
    pub btc_velocity: Option<f64>,
    pub btc_acceleration: Option<f64>,
    pub btc_volatility: Option<f64>,
}

/// Strategy executor - evaluates BTC price and generates trading signals
#[derive(Debug, Clone)]
pub struct StrategyExecutor {
    strategy_type: String,
    params: StrategyParams,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StrategyParams {
    pub min_delta: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub min_time_remaining: i64,
    pub max_time_remaining: i64,
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            min_delta: 0.0007,
            min_price: 0.30,
            max_price: 0.70,
            min_time_remaining: 3,
            max_time_remaining: 270,
        }
    }
}

impl StrategyExecutor {
    pub fn new(strategy_type: &str, params_json: &str) -> Self {
        let params: StrategyParams = serde_json::from_str(params_json).unwrap_or_default();
        tracing::debug!("Created {} strategy with min_delta={:.4}", strategy_type, params.min_delta);

        Self {
            strategy_type: strategy_type.to_string(),
            params,
        }
    }

    pub fn evaluate(&self, btc_price: f64, btc_change: Option<f64>) -> Signal {
        let ctx = StrategyContext {
            btc_price,
            btc_change,
            btc_window_open: None,
            yes_price: 0.5,
            no_price: 0.5,
            time_remaining: 60000,
            btc_velocity: None,
            btc_acceleration: None,
            btc_volatility: None,
        };
        self.evaluate_with_context(ctx)
    }

    pub fn evaluate_with_context(&self, ctx: StrategyContext) -> Signal {
        match self.strategy_type.as_str() {
            "window_delta" => self.evaluate_window_delta(ctx),
            "binance_signal" | "oracle_lag" => self.evaluate_oracle_lag(ctx),
            "last_seconds_scalp" => self.evaluate_last_seconds_scalp(ctx),
            "momentum" => self.evaluate_momentum(ctx),
            "trend" | "smart_trend" => self.evaluate_trend(ctx),
            "volatility" | "volatility_breakout" => self.evaluate_volatility(ctx),
            "sniper" => self.evaluate_sniper(ctx),
            "contrarian" => self.evaluate_contrarian(ctx),
            "mean_reversion" => self.evaluate_mean_reversion(ctx),
            "binance_velocity" | "velocity" => self.evaluate_velocity(ctx),
            "fair_value" => self.evaluate_fair_value(ctx),
            "price_reversion" => self.evaluate_price_reversion(ctx),
            "trend_pullback" => self.evaluate_trend_pullback(ctx),
            "ultra_low_entry" => self.evaluate_ultra_low_entry(ctx),
            "sniper_value" => self.evaluate_sniper_value(ctx),
            "odds_swing" => self.evaluate_odds_swing(ctx),
            "bayesian_ev" => self.evaluate_bayesian_ev(ctx),
            "high_conviction_momentum" => self.evaluate_high_conviction_momentum(ctx),
            "sniper_arb" => self.evaluate_sniper_arb(ctx),
            "volatility_filtered" => self.evaluate_volatility_filtered(ctx),
            "extreme_edge" => self.evaluate_extreme_edge(ctx),
            "yes_no_arb" => self.evaluate_yes_no_arb(ctx),
            "oracle_lag_v2" => self.evaluate_oracle_lag_v2(ctx),
            "low_volatility_edge" => self.evaluate_low_volatility_edge(ctx),
            "edge_hunter" => self.evaluate_edge_hunter(ctx),
            "strict_momentum" => self.evaluate_strict_momentum(ctx),
            "patient_waiter" => self.evaluate_patient_waiter(ctx),
            "signal_momentum_v2" => self.evaluate_signal_momentum_v2(ctx),
            _ => Signal::Hold(format!("Unknown strategy: {}", self.strategy_type)),
        }
    }

    fn check_price_limits(&self, action: &str, yes_price: f64, no_price: f64) -> bool {
        let target_price = if action == "YES" { yes_price } else { no_price };
        target_price >= self.params.min_price && target_price <= self.params.max_price
    }

    /// #1 WINDOW_DELTA - Compares BTC price vs window opening price
    fn evaluate_window_delta(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late to trade".to_string());
        }
        if ctx.time_remaining > self.params.max_time_remaining {
            return Signal::Hold("Window just started".to_string());
        }
        if ctx.btc_price == 0.0 {
            return Signal::Hold("No BTC price".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        if delta_pct > 0.12 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.70_f64 + (delta_pct - 0.12) * 3.0).min(0.92_f64);
            return Signal::Yes(confidence);
        }
        if delta_pct < -0.12 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.70_f64 + (-delta_pct - 0.12) * 3.0).min(0.92_f64);
            return Signal::No(confidence);
        }
        if delta_pct > 0.07 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + (delta_pct - 0.07) * 4.0).min(0.78_f64);
            return Signal::Yes(confidence);
        }
        if delta_pct < -0.07 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + (-delta_pct - 0.07) * 4.0).min(0.78_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("Delta too small: {:.4}%", delta_pct))
    }

    /// #2 ORACLE_LAG - Uses BTC change to predict market direction
    fn evaluate_oracle_lag(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c * 100.0,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 100.0 * 0.5;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change * 5.0).min(0.85_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change.abs() * 5.0).min(0.85_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("No oracle lag: {:.4}%", change))
        }
    }

    /// #3 LAST_SECONDS_SCALP - T-10 Sniper, only active in last 30 seconds
    fn evaluate_last_seconds_scalp(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining > 30 || ctx.time_remaining < 4 {
            return Signal::Hold("Outside T-10 window".to_string());
        }
        if ctx.btc_price == 0.0 {
            return Signal::Hold("No BTC price".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        if delta_pct.abs() < 0.04 {
            return Signal::Hold(format!("Delta too small: {:.4}%", delta_pct));
        }

        let action = if delta_pct > 0.0 { "YES" } else { "NO" };
        let target_price = if action == "YES" { ctx.yes_price } else { ctx.no_price };

        if target_price > 0.75 {
            return Signal::Hold(format!("Price too high: {:.0}c", target_price * 100.0));
        }
        if target_price < 0.25 {
            return Signal::Hold(format!("Price too low: {:.0}c", target_price * 100.0));
        }

        let confidence = 0.60_f64 + (delta_pct.abs() * 3.0).min(0.25_f64);
        if action == "YES" {
            Signal::Yes(confidence.min(0.85_f64))
        } else {
            Signal::No(confidence.min(0.85_f64))
        }
    }

    /// MOMENTUM - follows recent BTC direction
    fn evaluate_momentum(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        // Lower threshold: 0.0003 (0.03%) instead of min_delta
        let threshold = self.params.min_delta * 0.4;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change * 50.0).min(0.80_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change.abs() * 50.0).min(0.80_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("No momentum: {:.4}%", change * 100.0))
        }
    }

    /// SMART TREND - requires stronger confirmation
    fn evaluate_trend(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 1.0;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change * 40.0).min(0.82_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change.abs() * 40.0).min(0.82_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("No strong trend: {:.4}%", change * 100.0))
        }
    }

    /// VOLATILITY BREAKOUT - catches sudden moves in either direction
    fn evaluate_volatility(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 1.5;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change * 30.0).min(0.82_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change.abs() * 30.0).min(0.82_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("Low volatility: {:.4}%", change.abs() * 100.0))
        }
    }

    /// SNIPER - high confidence, low frequency
    fn evaluate_sniper(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 2.0;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            Signal::Yes(0.85)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            Signal::No(0.85)
        } else {
            Signal::Hold("Waiting for sniper setup".to_string())
        }
    }

    /// CONTRARIAN - bets against movement (mean reversion)
    fn evaluate_contrarian(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 0.5;

        // Price up -> bet NO (will revert), Price down -> bet YES (will revert)
        if change > threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change * 30.0).min(0.75_f64);
            Signal::No(confidence)
        } else if change < -threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change.abs() * 30.0).min(0.75_f64);
            Signal::Yes(confidence)
        } else {
            Signal::Hold("No contrarian signal".to_string())
        }
    }

    /// MEAN REVERSION - bets on return to 0.5 when price is extreme
    fn evaluate_mean_reversion(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        if ctx.yes_price > 0.72 {
            Signal::No(0.65)
        } else if ctx.yes_price < 0.28 {
            Signal::Yes(0.65)
        } else if ctx.yes_price > 0.62 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            Signal::No(0.55)
        } else if ctx.yes_price < 0.38 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            Signal::Yes(0.55)
        } else {
            Signal::Hold(format!("Price near fair value: {:.0}c", ctx.yes_price * 100.0))
        }
    }

    /// BINANCE VELOCITY - exact port from polymarket-demo binance-velocity.ts
    /// Uses BTC velocity (rate of change) and acceleration from Binance klines
    /// Only trades when BOTH velocity AND acceleration confirm the direction
    fn evaluate_velocity(&self, ctx: StrategyContext) -> Signal {
        // Time check - avoid last 45 seconds
        if ctx.time_remaining < 45 {
            return Signal::Hold("Too close to closure".to_string());
        }

        let velocity = ctx.btc_velocity.unwrap_or(0.0);
        let acceleration = ctx.btc_acceleration.unwrap_or(0.0);
        let btc_volatility = ctx.btc_volatility.unwrap_or(0.0);

        if ctx.btc_price == 0.0 {
            return Signal::Hold("No BTC price".to_string());
        }

        // Avoid high volatility periods (unpredictable)
        // btc_volatility > 0.003 means >0.3% volatility
        if btc_volatility > 0.003 {
            return Signal::Hold("High volatility - market unpredictable".to_string());
        }

        // Minimum velocity threshold: 0.015% per second
        let min_velocity: f64 = 0.00015;
        let min_acceleration: f64 = 0.00008;

        if velocity.abs() < min_velocity {
            return Signal::Hold(format!("Velocity too low: {:.4}%/s (choppy)", velocity * 100.0));
        }

        let is_up = velocity > 0.0;
        let is_accelerating = (is_up && acceleration > 0.0) || (!is_up && acceleration < 0.0);

        // Need BOTH velocity AND acceleration above thresholds
        if velocity.abs() < min_velocity || acceleration.abs() < min_acceleration {
            return Signal::Hold("Signal too weak - need both velocity AND acceleration".to_string());
        }

        // Only trade if accelerating (momentum building)
        if is_accelerating {
            // Check price limits
            let action = if is_up { "YES" } else { "NO" };
            let target_price = if is_up { ctx.yes_price } else { ctx.no_price };

            if target_price < self.params.min_price || target_price > self.params.max_price {
                return Signal::Hold(format!("Price out of range: {:.0}c", target_price * 100.0));
            }

            // Confidence calculation matching demo
            let vel_strength = (velocity.abs() * 800.0).min(0.25);
            let acc_boost = (acceleration.abs() * 800.0).min(0.15);
            let base_confidence = 0.55_f64;
            let confidence = (base_confidence + vel_strength + acc_boost).min(0.80);

            tracing::info!(
                "Binance Velocity: {} | vel={:.3}%/s acc={:.4}%/s² conf={:.2}",
                action, velocity * 100.0, acceleration * 100.0, confidence
            );

            if is_up {
                Signal::Yes(confidence)
            } else {
                Signal::No(confidence)
            }
        } else {
            // Decelerating - momentum fading, skip
            Signal::Hold(format!(
                "Decelerating - momentum fading: vel={:.3}%/s acc={:.4}%/s²",
                velocity * 100.0, acceleration * 100.0
            ))
        }
    }

    /// FAIR VALUE ARB - basic arbitrage around 0.5
    fn evaluate_fair_value(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = ctx.btc_change.unwrap_or(0.0);

        // Use BTC direction to confirm fair value bet
        if ctx.yes_price > 0.55 && ctx.yes_price <= self.params.max_price && change < 0.0 {
            Signal::No(0.58)
        } else if ctx.yes_price < 0.45 && ctx.yes_price >= self.params.min_price && change > 0.0 {
            Signal::Yes(0.58)
        } else if ctx.yes_price > 0.60 && ctx.yes_price <= self.params.max_price {
            Signal::No(0.52)
        } else if ctx.yes_price < 0.40 && ctx.yes_price >= self.params.min_price {
            Signal::Yes(0.52)
        } else {
            Signal::Hold(format!("Near fair value: {:.0}c", ctx.yes_price * 100.0))
        }
    }

    /// PRICE REVERSION - bets when price deviates significantly
    fn evaluate_price_reversion(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        if ctx.yes_price > 0.82 {
            Signal::No(0.70)
        } else if ctx.yes_price < 0.18 {
            Signal::Yes(0.70)
        } else if ctx.yes_price > 0.70 {
            Signal::No(0.62)
        } else if ctx.yes_price < 0.30 {
            Signal::Yes(0.62)
        } else if ctx.yes_price > 0.62 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            Signal::No(0.53)
        } else if ctx.yes_price < 0.38 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            Signal::Yes(0.53)
        } else {
            Signal::Hold(format!("No extreme price: {:.0}c", ctx.yes_price * 100.0))
        }
    }

    /// TREND PULLBACK - high-conviction hours only (00-02, 08-10, 14-16 UTC)
    fn evaluate_trend_pullback(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        let hour = {
            use chrono::Timelike;
            chrono::Utc::now().hour()
        };
        let is_high_conviction = matches!(hour, 0..=1 | 8..=9 | 14..=15 | 20..=21);
        if !is_high_conviction {
            return Signal::Hold(format!("Normal hour: {}:00 UTC", hour));
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        if delta_pct.abs() < 0.02 {
            return Signal::Hold("Delta too small".to_string());
        }

        if delta_pct > 0.0 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + delta_pct.abs() * 3.0).min(0.82_f64);
            Signal::Yes(confidence)
        } else if delta_pct < 0.0 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + delta_pct.abs() * 3.0).min(0.82_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold("Price out of range".to_string())
        }
    }

    /// ULTRA LOW ENTRY - buys at 4-15 cents where market underestimates probability
    fn evaluate_ultra_low_entry(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        let change = ctx.btc_change.unwrap_or(0.0);

        // Buy YES if very cheap and BTC is going up
        if ctx.yes_price < 0.15 && ctx.yes_price >= 0.04 && (delta_pct > 0.02 || change > 0.0001) {
            let confidence = (0.55_f64 + (0.15 - ctx.yes_price) * 3.0).min(0.85_f64);
            return Signal::Yes(confidence);
        }

        // Buy NO if very cheap and BTC is going down
        if ctx.no_price < 0.15 && ctx.no_price >= 0.04 && (delta_pct < -0.02 || change < -0.0001) {
            let confidence = (0.55_f64 + (0.15 - ctx.no_price) * 3.0).min(0.85_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("Not in ultra-low range: YES={:.0}c NO={:.0}c", ctx.yes_price * 100.0, ctx.no_price * 100.0))
    }

    /// SNIPER VALUE - buys at extremes
    fn evaluate_sniper_value(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }

        if ctx.yes_price < 0.15 {
            let confidence = (0.60_f64 + (0.15 - ctx.yes_price) * 3.0).min(0.90_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.15 {
            let confidence = (0.60_f64 + (0.15 - ctx.no_price) * 3.0).min(0.90_f64);
            return Signal::No(confidence);
        }

        if ctx.yes_price > 0.55 && ctx.yes_price < 0.70 {
            let confidence = (0.52_f64 + (ctx.yes_price - 0.55) * 2.0).min(0.72_f64);
            return Signal::No(confidence);
        }

        if ctx.yes_price < 0.45 && ctx.yes_price > 0.30 {
            let confidence = (0.52_f64 + (0.45 - ctx.yes_price) * 2.0).min(0.72_f64);
            return Signal::Yes(confidence);
        }

        Signal::Hold(format!("No sniper setup: {:.0}c", ctx.yes_price * 100.0))
    }

    /// ODDS SWING - buys outcomes priced < 15 cents for a swing to 2x
    fn evaluate_odds_swing(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }

        if ctx.yes_price < 0.15 && ctx.yes_price >= 0.04 {
            let confidence = (0.50_f64 + (0.15 - ctx.yes_price) * 4.0).min(0.80_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.15 && ctx.no_price >= 0.04 {
            let confidence = (0.50_f64 + (0.15 - ctx.no_price) * 4.0).min(0.80_f64);
            return Signal::No(confidence);
        }

        Signal::Hold("No swing opportunity".to_string())
    }

    /// BAYESIAN EV - requires 3 conditions: delta edge + market mispricing + Kelly positive EV
    fn evaluate_bayesian_ev(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        // Also consider tick change
        let tick_change = ctx.btc_change.unwrap_or(0.0) * 100.0;
        let combined_delta = delta_pct * 0.7 + tick_change * 0.3;

        if combined_delta.abs() < 0.03 {
            return Signal::Hold("Insufficient BTC delta".to_string());
        }

        let fair_up_prob = (0.5_f64 + (combined_delta / 0.05).tanh() * 0.45)
            .clamp(0.03, 0.97);
        let edge = fair_up_prob - ctx.yes_price;

        if edge.abs() < 0.05 {
            return Signal::Hold("Edge too small".to_string());
        }

        if edge > 0.05 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let b = if ctx.yes_price > 0.0 { (1.0 - ctx.yes_price) / ctx.yes_price } else { 1.0 };
            let kelly = (b * fair_up_prob - (1.0 - fair_up_prob)) / b;
            if kelly <= 0.0 {
                return Signal::Hold("Negative Kelly".to_string());
            }
            let confidence = (0.50_f64 + edge * 3.0).min(0.82_f64);
            Signal::Yes(confidence)
        } else if -edge > 0.05 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.50_f64 + (-edge) * 3.0).min(0.82_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold("No Bayesian edge".to_string())
        }
    }

    // ============================================================
    // NEW PROFITABLE STRATEGIES FOR LOW-VOLATILITY BTC MARKET
    // ============================================================

    /// HIGH_CONVICTION_MOMENTUM - Only trades when confidence > 0.75
    /// Key insight: Need >75% win rate to overcome the 50c edge
    /// Only triggers on STRONG momentum (+/- 0.15%+ BTC change)
    fn evaluate_high_conviction_momentum(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }
        if ctx.time_remaining > 240 {
            return Signal::Hold("Window just opened".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        // Strong threshold: 0.05% minimum (was 0.15%)
        let strong_threshold = 0.0005; // 0.05%

        if change > strong_threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            // Only trade if price is reasonable (not already inflated)
            if ctx.yes_price > 0.60 {
                return Signal::Hold(format!("YES price too high: {:.0}c", ctx.yes_price * 100.0));
            }
            let confidence = (0.75_f64 + change * 100.0).min(0.92_f64);
            if confidence >= 0.75 {
                return Signal::Yes(confidence);
            }
            return Signal::Hold(format!("Confidence too low: {:.2}", confidence));
        }

        if change < -strong_threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            if ctx.no_price > 0.60 {
                return Signal::Hold(format!("NO price too high: {:.0}c", ctx.no_price * 100.0));
            }
            let confidence = (0.75_f64 + change.abs() * 100.0).min(0.92_f64);
            if confidence >= 0.75 {
                return Signal::No(confidence);
            }
            return Signal::Hold(format!("Confidence too low: {:.2}", confidence));
        }

        Signal::Hold(format!("No strong momentum: {:.4}%", change * 100.0))
    }

    /// SNIPER_ARB - Extreme price deviation + BTC confirmation
    /// Only trades when YES/NO is 42c or less (big deviation from 50c)
    /// With BTC confirmation for direction
    fn evaluate_sniper_arb(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 240 {
            return Signal::Hold("Window just opened".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        // Only trade when market mispriced: YES < 0.42 or NO < 0.42
        // These are "cheap" and likely to revert to 50c

        // YES is cheap (< 42c) + BTC going UP = buy YES (will revert up)
        if ctx.yes_price < 0.42 && ctx.yes_price >= 0.30 {
            if change > 0.0005 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
                let edge = 0.50 - ctx.yes_price; // How much discount from fair value
                let confidence = (0.60_f64 + edge * 5.0 + change * 50.0).min(0.88_f64);
                return Signal::Yes(confidence);
            }
        }

        // NO is cheap (< 42c) + BTC going DOWN = buy NO (will revert up)
        if ctx.no_price < 0.42 && ctx.no_price >= 0.30 {
            if change < -0.0005 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
                let edge = 0.50 - ctx.no_price;
                let confidence = (0.60_f64 + edge * 5.0 + change.abs() * 50.0).min(0.88_f64);
                return Signal::No(confidence);
            }
        }

        // Also: YES > 58c = slightly expensive, BTC going DOWN = bet NO
        if ctx.yes_price > 0.58 && ctx.yes_price <= 0.70 {
            if change < -0.0005 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
                let confidence = (0.58_f64 + change.abs() * 50.0).min(0.80_f64);
                return Signal::No(confidence);
            }
        }

        Signal::Hold(format!(
            "No sniper arb: YES={:.0}c NO={:.0}c",
            ctx.yes_price * 100.0,
            ctx.no_price * 100.0
        ))
    }

    /// VOLATILITY_FILTERED - Only trades when volatility is "sweet spot"
    /// Too low = random walk, Too high = unpredictable
    /// Sweet spot: 0.02% - 0.25% per 5-min window
    fn evaluate_volatility_filtered(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 240 {
            return Signal::Hold("Window just opened".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let abs_change = change.abs();

        // Sweet spot: 0.03% to 0.15% volatility (tightened)
        let min_vol = 0.0003; // 0.03%
        let max_vol = 0.0015; // 0.15%

        if abs_change < min_vol {
            return Signal::Hold(format!("Volatility too low: {:.4}%", abs_change * 100.0));
        }
        if abs_change > max_vol {
            return Signal::Hold(format!("Volatility too high: {:.4}%", abs_change * 100.0));
        }

        // Sweet spot - calculate confidence based on consistency
        let confidence = 0.62_f64 + (abs_change / max_vol) * 0.25_f64;

        if change > 0.0 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            // Only buy YES if price is reasonable
            if ctx.yes_price > 0.65 {
                return Signal::Hold(format!("YES price too high: {:.0}c", ctx.yes_price * 100.0));
            }
            return Signal::Yes(confidence.min(0.85_f64));
        }

        if change < 0.0 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            if ctx.no_price > 0.65 {
                return Signal::Hold(format!("NO price too high: {:.0}c", ctx.no_price * 100.0));
            }
            return Signal::No(confidence.min(0.85_f64));
        }

        Signal::Hold(format!("No clear direction: {:.4}%", change * 100.0))
    }

    /// EXTREME_EDGE - Only trades at EXTREME odds (>65c or <35c)
    /// Bets AGAINST the crowd when odds are at extremes
    /// Key insight: 5-min markets rarely stay at >65c or <35c for long
    /// Target: buy at 30-35c, sell at 50c = 43-67% gain per trade
    /// Even 1-in-3 accuracy would be profitable
    fn evaluate_extreme_edge(&self, ctx: StrategyContext) -> Signal {
        // Time check: trade between 30s and 250s (avoid first 30s open and last 30s close)
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 250 {
            return Signal::Hold("Window just opened".to_string());
        }

        // EXTREME OVERPRICED: YES > 68c → market thinks BTC will go up too much
        // This is a gift to bet against (BTC rarely moves that much in 5 min)
        if ctx.yes_price > 0.68 {
            let edge = ctx.yes_price - 0.50;
            // Confidence based on how extreme the mispricing is
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: YES overpriced at {:.0}c, betting NO conf={:.2}", ctx.yes_price * 100.0, confidence);
            return Signal::No(confidence);
        }

        // EXTREME UNDERPRICED: YES < 32c → market thinks BTC will go down too much
        // BTC rarely drops enough to make NO win
        if ctx.yes_price < 0.32 {
            let edge = 0.50 - ctx.yes_price;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: YES underpriced at {:.0}c, betting YES conf={:.2}", ctx.yes_price * 100.0, confidence);
            return Signal::Yes(confidence);
        }

        // Also check NO extremes
        if ctx.no_price > 0.68 {
            let edge = ctx.no_price - 0.50;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: NO overpriced at {:.0}c, betting YES conf={:.2}", ctx.no_price * 100.0, confidence);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.32 {
            let edge = 0.50 - ctx.no_price;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: NO underpriced at {:.0}c, betting NO conf={:.2}", ctx.no_price * 100.0, confidence);
            return Signal::No(confidence);
        }

        // Slightly extreme: >62c or <38c - lower confidence
        if ctx.yes_price > 0.62 {
            let edge = ctx.yes_price - 0.50;
            let confidence = (0.52_f64 + edge * 2.0).min(0.70_f64);
            return Signal::No(confidence);
        }

        if ctx.yes_price < 0.38 {
            let edge = 0.50 - ctx.yes_price;
            let confidence = (0.52_f64 + edge * 2.0).min(0.70_f64);
            return Signal::Yes(confidence);
        }

        Signal::Hold(format!(
            "No extreme edge: YES={:.0}c NO={:.0}c",
            ctx.yes_price * 100.0,
            ctx.no_price * 100.0
        ))
    }

    // ══════════════════════════════════════════════════════════════════════════════════
    // NEW STRATEGIES - Based on GitHub Research (PolymarketBtcBot + YES+NO_Arb)
    // ══════════════════════════════════════════════════════════════════════════════════

    /// YES_NO_ARB - Buys BOTH sides when combined price < $0.97
    /// Guaranteed profit because exactly one side resolves to $1.00
    /// Example: UP=$0.49, DOWN=$0.49 → cost=$0.98, payout=$1.00, profit=$0.02
    fn evaluate_yes_no_arb(&self, ctx: StrategyContext) -> Signal {
        // Time check: don't enter in last 30 seconds or first 20 seconds
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 280 {
            return Signal::Hold("Window just opened".to_string());
        }

        let combined = ctx.yes_price + ctx.no_price;

        // STRONG arbitrage opportunity: combined < $0.95
        if combined < 0.95 {
            tracing::info!(
                "YES_NO_ARB STRONG: YES={:.0}c + NO={:.0}c = {:.2} < $0.95 → Guaranteed profit {:.2}/share",
                ctx.yes_price * 100.0,
                ctx.no_price * 100.0,
                combined,
                1.0 - combined
            );
            // Confidence based on how deep the arb is
            let confidence = (0.60_f64 + (0.95 - combined) * 8.0).min(0.92_f64);
            // This is a NEUTRAL arb - we buy both sides, but Signal is just for logging
            // In actual implementation, this would place TWO orders
            return Signal::Yes(confidence); // Using Yes to indicate "both sides" for now
        }

        // MODERATE arb: combined < $0.97
        if combined < 0.97 {
            tracing::info!(
                "YES_NO_ARB MODERATE: YES={:.0}c + NO={:.0}c = {:.2} < $0.97",
                ctx.yes_price * 100.0,
                ctx.no_price * 100.0,
                combined
            );
            let confidence = (0.55_f64 + (0.97 - combined) * 5.0).min(0.80_f64);
            return Signal::Yes(confidence);
        }

        // WEAK arb: combined < $0.98
        if combined < 0.98 {
            let confidence = (0.52_f64 + (0.98 - combined) * 3.0).min(0.70_f64);
            return Signal::Yes(confidence);
        }

        Signal::Hold(format!(
            "No arb opportunity: YES={:.0}c + NO={:.0}c = {:.2} > $0.98",
            ctx.yes_price * 100.0,
            ctx.no_price * 100.0,
            combined
        ))
    }

    /// ORACLE_LAG_V2 - Improved version with tighter thresholds for BTC 5-min
    ///
    /// Key insight from PolymarketBtcBot research:
    /// - Chainlink oracle updates 10-45 seconds behind CEX
    /// - BTC 5-min windows: need only 0.03-0.08% move for edge
    /// - Confidence based on: lag duration + price divergence + exchange agreement
    fn evaluate_oracle_lag_v2(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 260 {
            return Signal::Hold("Window just opened".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let change_pct = change * 100.0;
        let abs_change = change_pct.abs();

        // TIGHTER thresholds for BTC 5-min (BTC only moves 0.02-0.03% typically)
        // Ideal range: 0.03% to 0.20% (sweet spot for oracle lag)
        let min_change = 0.03_f64;
        let ideal_min = 0.05_f64;
        let ideal_max = 0.20_f64;

        if abs_change < min_change {
            return Signal::Hold(format!("Change too small: {:.4}% (need >{:.2}%)", abs_change, min_change));
        }

        // STRONG signal: in ideal range with significant move
        if abs_change >= ideal_min && abs_change <= ideal_max {
            let confidence = if abs_change >= 0.10 {
                // Strong move: >= 0.10% → high confidence
                (0.68_f64 + abs_change * 1.5).min(0.88_f64)
            } else {
                // Moderate move: 0.05-0.10% → medium confidence
                (0.58_f64 + (abs_change - ideal_min) * 4.0).min(0.78_f64)
            };

            if change > 0.0 {
                tracing::info!(
                    "ORACLE_LAG_V2: BTC up {:.4}% in window → betting YES conf={:.2}",
                    change_pct, confidence
                );
                return Signal::Yes(confidence);
            } else {
                tracing::info!(
                    "ORACLE_LAG_V2: BTC down {:.4}% in window → betting NO conf={:.2}",
                    change_pct, confidence
                );
                return Signal::No(confidence);
            }
        }

        // WEAK signal: move exists but outside ideal range
        if abs_change >= min_change {
            let confidence = (0.52_f64 + abs_change * 2.0).min(0.68_f64);
            if change > 0.0 {
                return Signal::Yes(confidence);
            } else {
                return Signal::No(confidence);
            }
        }

        Signal::Hold(format!("No oracle lag: {:.4}%", change_pct))
    }

    /// LOW_VOLATILITY_EDGE - Tuned for BTC's low volatility environment
    ///
    /// Key insight from research:
    /// - BTC 5-min windows: typically 0.02-0.05% move
    /// - At 50c odds: need >50.5% win rate just to break even (due to spread)
    /// - Strategy: ONLY trade when:
    ///   1. Clear directional move (even if small)
    ///   2. Price is NOT at 50c (extreme odds give better risk/reward)
    ///   3. Momentum is CONSISTENT (not choppy)
    fn evaluate_low_volatility_edge(&self, ctx: StrategyContext) -> Signal {
        // Trade window: 30s to 270s (avoid open chaos and close risk)
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 270 {
            return Signal::Hold("Window just opened".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let change_pct = change * 100.0;
        let abs_change = change_pct.abs();

        // BTC-specific thresholds: very low, but meaningful
        let noise_threshold = 0.01_f64;  // <0.01% = noise
        let min_signal = 0.02_f64;       // >=0.02% = potential signal
        let strong_signal = 0.06_f64;    // >=0.06% = strong signal

        if abs_change < noise_threshold {
            return Signal::Hold(format!("Price noise: {:.4}%", abs_change));
        }

        // Check price reasonability - don't buy at 50c
        // Need price to be at least 2 cents away from 50
        let yes_edge = (ctx.yes_price - 0.50).abs();
        let no_edge = (ctx.no_price - 0.50).abs();
        let min_edge = 0.02_f64;

        if yes_edge < min_edge && no_edge < min_edge {
            return Signal::Hold(format!(
                "Price too close to 50c: YES={:.0}c NO={:.0}c",
                ctx.yes_price * 100.0,
                ctx.no_price * 100.0
            ));
        }

        // Calculate confidence based on multiple factors
        let mut confidence = 0.55_f64;

        // Factor 1: Move strength
        if abs_change >= strong_signal {
            confidence += 0.18_f64;
        } else if abs_change >= 0.04_f64 {
            confidence += 0.12_f64;
        } else if abs_change >= min_signal {
            confidence += 0.06_f64;
        }

        // Factor 2: Price edge (better odds = higher confidence)
        if yes_edge >= 0.08_f64 || no_edge >= 0.08_f64 {
            confidence += 0.10_f64;
        } else if yes_edge >= 0.05_f64 || no_edge >= 0.05_f64 {
            confidence += 0.06_f64;
        }

        // Factor 3: Time remaining (better in middle of window)
        let mid_window = 150_f64; // 150 seconds = middle
        let time_from_mid = (ctx.time_remaining as f64 - mid_window).abs();
        if time_from_mid < 60_f64 {
            confidence += 0.05_f64; // Near middle of window
        }

        confidence = confidence.min(0.82_f64);

        // Execute on direction
        if change > 0.0 && ctx.yes_price < 0.75 {
            tracing::info!(
                "LOW_VOL_EDGE: BTC up {:.4}%, YES={:.0}c → betting YES conf={:.2}",
                change_pct,
                ctx.yes_price * 100.0,
                confidence
            );
            return Signal::Yes(confidence);
        }

        if change < 0.0 && ctx.no_price < 0.75 {
            tracing::info!(
                "LOW_VOL_EDGE: BTC down {:.4}%, NO={:.0}c → betting NO conf={:.2}",
                change_pct,
                ctx.no_price * 100.0,
                confidence
            );
            return Signal::No(confidence);
        }

        Signal::Hold(format!(
            "No low-vol edge: change={:.4}%, YES={:.0}c NO={:.0}c",
            change_pct,
            ctx.yes_price * 100.0,
            ctx.no_price * 100.0
        ))
    }

    /// EDGE_HUNTER - Only trades when our probability > market probability by margin
    ///
    /// Key insight: Trade when we have a mathematical edge
    /// Calculates fair probability from BTC delta, compares to market price
    fn evaluate_edge_hunter(&self, ctx: StrategyContext) -> Signal {
        // Time window: 20s to 260s
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 260 {
            return Signal::Hold("Window just opened".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        // Need minimum BTC movement for calculation to be meaningful
        let min_delta = 0.05_f64; // 0.05%
        if delta_pct.abs() < min_delta {
            return Signal::Hold(format!("Delta {:.3}% < {:.2}% threshold", delta_pct, min_delta));
        }

        // Calculate fair probability from delta using tanh
        let our_prob = (0.5_f64 + (delta_pct / 0.05).tanh() * 0.45).clamp(0.05, 0.95);
        let market_prob = ctx.yes_price;

        // Calculate edge
        let edge = our_prob - market_prob;
        let min_edge = 0.03_f64; // Need 3% edge minimum

        if edge > min_edge {
            // Our probability > market - market is undervaluing YES
            let confidence = (0.55_f64 + edge * 3.0).min(0.82_f64);
            tracing::info!(
                "EDGE_HUNTER: our {:.1}% > market {:.1}% → YES conf={:.2}",
                our_prob * 100.0,
                market_prob * 100.0,
                confidence
            );
            return Signal::Yes(confidence);
        }

        if edge < -min_edge {
            // Market thinks YES is MORE likely than we do - bet NO
            let confidence = (0.55_f64 + (-edge) * 3.0).min(0.82_f64);
            tracing::info!(
                "EDGE_HUNTER: our {:.1}% < market {:.1}% → NO conf={:.2}",
                our_prob * 100.0,
                market_prob * 100.0,
                confidence
            );
            return Signal::No(confidence);
        }

        Signal::Hold(format!(
            "No edge: our {:.1}% vs market {:.1}% (need {:.1}%)",
            our_prob * 100.0,
            market_prob * 100.0,
            min_edge * 100.0
        ))
    }

    /// STRICT_MOMENTUM - Only trades on VERY strong BTC moves
    ///
    /// Key insight: Small momentum is just noise. Only trade when BTC moves >0.15%
    /// Significantly reduces false signals and improves win rate
    fn evaluate_strict_momentum(&self, ctx: StrategyContext) -> Signal {
        // Time window: 20s to 260s
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 260 {
            return Signal::Hold("Window just opened".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let change_pct = change * 100.0;
        let abs_change = change_pct.abs();

        // STRICT threshold - only strong moves
        let threshold = 0.15_f64; // 0.15% - much higher than typical

        if abs_change < threshold {
            return Signal::Hold(format!(
                "Delta {:.3}% < {:.2}% threshold (noise)",
                abs_change, threshold
            ));
        }

        // Check price limits - avoid extreme odds
        if ctx.yes_price < 0.35 || ctx.yes_price > 0.65 {
            return Signal::Hold(format!(
                "PM price {:.1}% outside [35-65%] range",
                ctx.yes_price * 100.0
            ));
        }

        // Strong move detected
        if change > 0.0 {
            let confidence = (0.65_f64 + abs_change * 1.5).min(0.88_f64);
            tracing::info!(
                "STRICT_MOMENTUM: BTC +{:.3}% → YES conf={:.2}",
                change_pct, confidence
            );
            return Signal::Yes(confidence);
        } else {
            let confidence = (0.65_f64 + abs_change * 1.5).min(0.88_f64);
            tracing::info!(
                "STRICT_MOMENTUM: BTC {:.3}% → NO conf={:.2}",
                change_pct, confidence
            );
            return Signal::No(confidence);
        }
    }

    /// PATIENT_WAITER - Waits for PERFECT setups only
    ///
    /// Key insight: Don't trade just to trade. Wait for conditions where:
    /// 1. Odds are near 50% (maximum expected value zone)
    /// 2. BTC has moved enough to give directional conviction
    /// Trades VERY infrequently but only when odds are in our favor
    fn evaluate_patient_waiter(&self, ctx: StrategyContext) -> Signal {
        // Time window: 30s to 270s
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 270 {
            return Signal::Hold("Window just opened".to_string());
        }

        // KEY: Only trade when odds are near 50% (sweet spot)
        let distance_from_50 = (ctx.yes_price - 0.5).abs();
        let max_distance = 0.05_f64; // Within 5% of 50c

        if distance_from_50 > max_distance {
            return Signal::Hold(format!(
                "Odds {:.1}% not in 45-55% sweet spot",
                ctx.yes_price * 100.0
            ));
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        // Need clear directional move
        let min_delta = 0.12_f64; // 0.12%
        if delta_pct.abs() < min_delta {
            return Signal::Hold(format!(
                "BTC delta {:.3}% < {:.2}% (no clear direction)",
                delta_pct, min_delta
            ));
        }

        // Perfect setup: near 50c odds + clear BTC move
        if delta_pct > 0.0 {
            let confidence = (0.60_f64 + delta_pct * 2.0).min(0.85_f64);
            tracing::info!(
                "PATIENT_WAITER: BTC +{:.3}%, YES@{:.1}% → YES conf={:.2}",
                delta_pct,
                ctx.yes_price * 100.0,
                confidence
            );
            return Signal::Yes(confidence);
        } else {
            let confidence = (0.60_f64 + (-delta_pct) * 2.0).min(0.85_f64);
            tracing::info!(
                "PATIENT_WAITER: BTC {:.3}%, NO@{:.1}% → NO conf={:.2}",
                delta_pct,
                ctx.no_price * 100.0,
                confidence
            );
            return Signal::No(confidence);
        }
    }

    /// SIGNAL_MOMENTUM_V2 - Improved momentum with better risk calibration
    ///
    /// Key improvements:
    /// 1. Higher delta threshold (0.20%) to filter noise
    /// 2. Lower base confidence (0.55) for better calibrated risk
    /// 3. Time decay: reduce confidence as market approaches close
    /// 4. Volatility-adjusted: need minimum volatility for signal
    ///
    /// Rules:
    /// - BTC delta >= 0.20% OR <= -0.20% (otherwise HOLD)
    /// - Confidence = 0.55 + (delta_pct / 0.3) * 0.20, capped at 0.75
    /// - Time remaining: 30s to 250s
    /// - Price range: 0.30 to 0.70
    fn evaluate_signal_momentum_v2(&self, ctx: StrategyContext) -> Signal {
        // Time window: 30s to 250s
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 250 {
            return Signal::Hold("Window just opened".to_string());
        }

        // Price check: only trade 30-70c range
        if ctx.yes_price < 0.30 || ctx.yes_price > 0.70 {
            return Signal::Hold(format!(
                "Price {:.0}c outside 30-70c range",
                ctx.yes_price * 100.0
            ));
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        let abs_delta = delta_pct.abs();

        // STRICT gate: need significant BTC movement (0.20%+)
        let min_delta = 0.20_f64;
        if abs_delta < min_delta {
            return Signal::Hold(format!(
                "BTC delta {:.3}% < {:.2}% threshold (noise)",
                delta_pct, min_delta
            ));
        }

        // Calculate time decay factor (0.9 to 1.0)
        let time_factor = if ctx.time_remaining > 150 {
            1.0  // Plenty of time
        } else if ctx.time_remaining > 60 {
            0.95  // Getting close
        } else {
            0.90  // Very close to close
        };

        // Calculate confidence based on delta strength
        // Delta 0.20% -> 0.55 confidence
        // Delta 0.35% -> 0.65
        // Delta 0.50% -> 0.75 (cap)
        let base_confidence = 0.55_f64;
        let max_confidence = 0.75_f64;
        let delta_strength = (abs_delta - min_delta).min(0.30);
        let raw_confidence =
            base_confidence + (delta_strength / 0.30) * (max_confidence - base_confidence);
        let confidence = (raw_confidence * time_factor).min(max_confidence);

        // Execute trade based on direction
        if delta_pct > 0.0 {
            tracing::info!(
                "MOMENTUM_V2: BTC +{:.3}%, YES@{:.0}c conf={:.2}",
                delta_pct,
                ctx.yes_price * 100.0,
                confidence
            );
            Signal::Yes(confidence)
        } else {
            tracing::info!(
                "MOMENTUM_V2: BTC {:.3}%, NO@{:.0}c conf={:.2}",
                delta_pct,
                (1.0 - ctx.yes_price) * 100.0,
                confidence
            );
            Signal::No(confidence)
        }
    }
}
