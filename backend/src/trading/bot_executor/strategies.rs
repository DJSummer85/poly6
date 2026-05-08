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
}
