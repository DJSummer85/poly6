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
    pub market_start_price: Option<f64>,
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
            market_start_price: None,
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
            time_remaining: self.time_remaining,
            btc_velocity: self.btc_velocity,
            btc_acceleration: self.btc_acceleration,
            btc_volatility: self.btc_volatility,
            market_start_price: self.market_start_price,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    Yes(f64),     // Buy YES, confidence 0-1
    No(f64),      // Buy NO (sell YES), confidence 0-1
    Hold(String), // No action, reason
}

/// Strategy context - all data needed for strategy evaluation
#[derive(Debug, Clone)]
pub struct StrategyContext {
    pub btc_price: f64,
    pub btc_change: Option<f64>,   // ~30 second window change (decimal, e.g. 0.001 = 0.1%)
    pub btc_window_open: Option<f64>,
    pub yes_price: f64,
    pub no_price: f64,
    pub time_remaining: i64,       // SECONDS
    pub btc_velocity: Option<f64>, // % change per second
    pub btc_acceleration: Option<f64>,
    pub btc_volatility: Option<f64>,
    pub market_start_price: Option<f64>,
}

/// Strategy executor - evaluates BTC price and generates trading signals
#[derive(Debug, Clone)]
pub struct StrategyExecutor {
    pub strategy_type: String,
    pub params: StrategyParams,
}

/// FIX: min_delta properly calibrated for 30-second window
/// Default: 0.001 (0.1% / 30sec) - realistic movement threshold
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StrategyParams {
    pub min_delta: f64,
    pub min_price: f64,
    pub max_price: f64,
    pub min_time_remaining: i64, // seconds
    pub max_time_remaining: i64, // seconds
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            min_delta: 0.0008,       // 0.08% — nyugodt piacon is beléphet, de nem zajra
            min_price: 0.30,
            max_price: 0.70,
            min_time_remaining: 15,  // FIX: 15 másodperc minimum (volt: 8)
            max_time_remaining: 270, // 4.5 perc maximum
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
            time_remaining: 60,
            btc_velocity: None,
            btc_acceleration: None,
            btc_volatility: None,
            market_start_price: None,
        };
        self.evaluate_with_context(ctx)
    }

    pub fn evaluate_with_context(&self, ctx: StrategyContext) -> Signal {
        if ctx.btc_price == 0.0 {
            return Signal::Hold("No price data".into());
        }
        
        // Wait for at least some history if we need change (handled in strategies)
        
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

        // Strong signal: delta > 0.18% → higher confidence, less noise
        if delta_pct > 0.18 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.74_f64 + (delta_pct - 0.18) * 2.0).min(0.92_f64);
            return Signal::Yes(confidence);
        }
        if delta_pct < -0.18 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.74_f64 + (-delta_pct - 0.18) * 2.0).min(0.92_f64);
            return Signal::No(confidence);
        }
        // Medium signal: delta > 0.10% → requires conviction
        if delta_pct > 0.10 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.58_f64 + (delta_pct - 0.10) * 2.0).min(0.78_f64);
            return Signal::Yes(confidence);
        }
        if delta_pct < -0.10 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.58_f64 + (-delta_pct - 0.10) * 2.0).min(0.78_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("Delta too small: {:.4}%", delta_pct))
    }

    fn evaluate_oracle_lag(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c * 100.0,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 100.0;

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

    fn evaluate_last_seconds_scalp(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining > 30 || ctx.time_remaining < 6 {
            return Signal::Hold("Outside scalp window".to_string());
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

        if delta_pct.abs() < 0.06 {
            return Signal::Hold(format!("Delta too small: {:.4}%", delta_pct));
        }

        let action = if delta_pct > 0.0 { "YES" } else { "NO" };
        let target_price = if action == "YES" { ctx.yes_price } else { ctx.no_price };

        if target_price > 0.70 {
            return Signal::Hold(format!("Price too high: {:.0}c", target_price * 100.0));
        }
        if target_price < 0.30 {
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
    /// FIX: Higher threshold and confidence baseline
    fn evaluate_momentum(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        // Kiszámoljuk, hogy az indulás óta (window_open) merre tart az ár
        let window_delta = if let (Some(open), true) = (ctx.btc_window_open, ctx.btc_price > 0.0) {
            (ctx.btc_price - open) / open
        } else {
            0.0
        };

        // Overextended filter: if move is > 0.50%, it's likely to revert
        let overextended_threshold = 0.0050;
        let threshold = self.params.min_delta * 1.2;

        if change.abs() > overextended_threshold {
            return Signal::Hold(format!("Overextended: {:.3}%", change * 100.0));
        }

        // Csak akkor kötünk momentumra, ha a rövid távú lendület MEGEGYEZIK a piac indulása óta tartó fő iránnyal.
        // Különben simán bekötnénk egy mikro-korrekcióba, ami garantált veszteség.
        let is_primary_trend_up = window_delta > -0.0005; // Megengedünk egy pici zajt
        let is_primary_trend_down = window_delta < 0.0005;

        if change > threshold && is_primary_trend_up && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.56_f64 + change.abs() * 45.0 + window_delta.max(0.0) * 10.0).min(0.82_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && is_primary_trend_down && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.54_f64 + change.abs() * 40.0 + (-window_delta).max(0.0) * 10.0).min(0.78_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("No safe momentum: {:.4}% (window: {:.4}%)", change * 100.0, window_delta * 100.0))
        }
    }

    fn evaluate_trend(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let window_delta = if let (Some(open), true) = (ctx.btc_window_open, ctx.btc_price > 0.0) {
            (ctx.btc_price - open) / open
        } else {
            0.0
        };

        let threshold = self.params.min_delta * 1.5;
        let is_primary_trend_up = window_delta > -0.0005;
        let is_primary_trend_down = window_delta < 0.0005;

        if change > threshold && is_primary_trend_up && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change * 60.0).min(0.80_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && is_primary_trend_down && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change.abs() * 60.0).min(0.80_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("No safe trend: {:.4}%", change * 100.0))
        }
    }

    fn evaluate_volatility(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 2.0;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change * 100.0).min(0.82_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change.abs() * 100.0).min(0.82_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("Low volatility: {:.4}%", change.abs() * 100.0))
        }
    }

    fn evaluate_sniper(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 1.5;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change.abs() * 80.0).min(0.85_f64);
            Signal::Yes(confidence)
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.60_f64 + change.abs() * 80.0).min(0.85_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold("Waiting for sniper setup".to_string())
        }
    }

    fn evaluate_contrarian(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = match ctx.btc_change {
            Some(c) => c,
            None => return Signal::Hold("No BTC data".to_string()),
        };

        let threshold = self.params.min_delta * 2.0;

        if change > threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change * 80.0).min(0.75_f64);
            Signal::No(confidence)
        } else if change < -threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + change.abs() * 80.0).min(0.75_f64);
            Signal::Yes(confidence)
        } else {
            Signal::Hold("No contrarian signal".to_string())
        }
    }

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

    fn evaluate_velocity(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to closure".to_string());
        }

        let velocity = ctx.btc_velocity.unwrap_or(0.0);
        let acceleration = ctx.btc_acceleration.unwrap_or(0.0);
        let btc_volatility = ctx.btc_volatility.unwrap_or(0.0);

        if ctx.btc_price == 0.0 {
            return Signal::Hold("No BTC price".to_string());
        }

        if btc_volatility > 0.003 {
            return Signal::Hold("High volatility - market unpredictable".to_string());
        }

        let min_velocity: f64 = 0.00002;
        let min_acceleration: f64 = 0.000002;

        // Only hold when BOTH are weak (ÉS kapcsolat helyett VAGY)
        if velocity.abs() < min_velocity && acceleration.abs() < min_acceleration {
            return Signal::Hold(format!(
                "Signal too weak: vel={:.4}%/s acc={:.5}%/s²",
                velocity * 100.0, acceleration * 100.0
            ));
        }

        let is_up = velocity > 0.0;
        let has_velocity = velocity.abs() >= min_velocity;

        // Direction: velocity if strong enough, otherwise acceleration trend
        let action = if has_velocity {
            if is_up { "YES" } else { "NO" }
        } else {
            if acceleration > 0.0 { "YES" } else { "NO" }
        };
        let target_price = if action == "YES" { ctx.yes_price } else { ctx.no_price };

        if target_price < self.params.min_price || target_price > self.params.max_price {
            return Signal::Hold(format!("Price out of range: {:.0}c", target_price * 100.0));
        }

        let is_accelerating = (is_up && acceleration > 0.0) || (!is_up && acceleration < 0.0);
        let vel_strength = (velocity.abs() * 800.0).min(0.25);
        let acc_boost = if is_accelerating { (acceleration.abs() * 10000.0).min(0.15) } else { 0.0 };
        let confidence = (0.55_f64 + vel_strength + acc_boost).min(0.80);

        tracing::info!(
            "Velocity signal: {} | vel={:.3}%/s acc={:.4}%/s² conf={:.2}",
            action, velocity * 100.0, acceleration * 100.0, confidence
        );

        if action == "YES" { Signal::Yes(confidence) } else { Signal::No(confidence) }
    }

    fn evaluate_fair_value(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        let change = ctx.btc_change.unwrap_or(0.0);
        let min_change = self.params.min_delta * 0.5;

        // Also consider the full window delta (BTC change from market open)
        let window_delta = ctx.btc_window_open.map(|open| {
            (ctx.btc_price - open) / open
        }).unwrap_or(0.0);

        // Combine short-term change with window trend for stronger signals
        let has_short_term = change.abs() > min_change;
        let has_window_trend = window_delta.abs() > self.params.min_delta;
        let both_confirm = has_short_term && has_window_trend && (change.signum() == window_delta.signum());

        if ctx.yes_price > 0.55 && ctx.yes_price <= self.params.max_price && (change < -min_change || window_delta < -self.params.min_delta) {
            let conf = if both_confirm { 0.64 } else { 0.55 };
            Signal::No(conf)
        } else if ctx.yes_price < 0.45 && ctx.yes_price >= self.params.min_price && (change > min_change || window_delta > self.params.min_delta) {
            let conf = if both_confirm { 0.64 } else { 0.55 };
            Signal::Yes(conf)
        // Extrém árfekvés: gyengébb BTC megerősítés is elég (csak short-term change, fele akkora küszöb)
        } else if ctx.yes_price > 0.62 && ctx.yes_price <= self.params.max_price && change < -min_change * 0.5 {
            Signal::No(0.55)
        } else if ctx.yes_price < 0.38 && ctx.yes_price >= self.params.min_price && change > min_change * 0.5 {
            Signal::Yes(0.55)
        } else {
            Signal::Hold(format!("Near fair value: {:.0}c", ctx.yes_price * 100.0))
        }
    }

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

    fn evaluate_trend_pullback(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        if delta_pct.abs() < 0.05 {
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

    fn evaluate_ultra_low_entry(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }

        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        let change = ctx.btc_change.unwrap_or(0.0);
        let min_change = self.params.min_delta * 0.5;

        if ctx.yes_price < 0.25 && ctx.yes_price >= 0.04 && (delta_pct > 0.05 || change > min_change) {
            let confidence = (0.55_f64 + (0.25 - ctx.yes_price) * 2.0).min(0.85_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.25 && ctx.no_price >= 0.04 && (delta_pct < -0.05 || change < -min_change) {
            let confidence = (0.55_f64 + (0.25 - ctx.no_price) * 2.0).min(0.85_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("Not in ultra-low range: YES={:.0}c NO={:.0}c", ctx.yes_price * 100.0, ctx.no_price * 100.0))
    }

    fn evaluate_sniper_value(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }

        let change = ctx.btc_change.unwrap_or(0.0);
        let min_change = self.params.min_delta * 0.5;

        if ctx.yes_price < 0.25 && ctx.yes_price >= 0.04 && change > min_change {
            let confidence = (0.60_f64 + (0.25 - ctx.yes_price) * 2.0).min(0.90_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.25 && ctx.no_price >= 0.04 && change < -min_change {
            let confidence = (0.60_f64 + (0.25 - ctx.no_price) * 2.0).min(0.90_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("No sniper setup: YES={:.0}c NO={:.0}c", ctx.yes_price * 100.0, ctx.no_price * 100.0))
    }

    fn evaluate_odds_swing(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }

        let change = ctx.btc_change.unwrap_or(0.0);
        let min_change = self.params.min_delta * 0.5;

        if ctx.yes_price < 0.25 && ctx.yes_price >= 0.04 && change > min_change {
            let confidence = (0.55_f64 + (0.25 - ctx.yes_price) * 2.0).min(0.80_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.25 && ctx.no_price >= 0.04 && change < -min_change {
            let confidence = (0.55_f64 + (0.25 - ctx.no_price) * 2.0).min(0.80_f64);
            return Signal::No(confidence);
        }

        Signal::Hold("No swing opportunity".to_string())
    }

    fn evaluate_bayesian_ev(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
        }

        // Calculate BTC delta and signal strength
        let window_open = ctx.btc_window_open.unwrap_or(ctx.btc_price);
        let delta_pct = if window_open > 0.0 {
            ((ctx.btc_price - window_open) / window_open) * 100.0
        } else {
            0.0
        };

        let tick_change = ctx.btc_change.unwrap_or(0.0) * 100.0;
        let combined_delta = delta_pct * 0.8 + tick_change * 0.2;

        if combined_delta.abs() < 0.02 {
            return Signal::Hold("Insufficient BTC delta".to_string());
        }

        // Bayesian prior: 0.50 (neutral for 5-min BTC market)
        // Signal strength from combined delta (0.0 to 1.0)
        let signal_strength = (combined_delta.abs() / 0.30).clamp(0.0, 1.0); // 0.30% = max signal

        // Likelihood: how informative is the signal
        // strength=0.0 → likelihood=0.50 (uninformative)
        // strength=1.0 → likelihood=0.90 (very informative)
        let likelihood = 0.50 + signal_strength * 0.40;

        // Bayesian posterior: P(side | signal) = prior * likelihood / evidence
        let prior_up = 0.50;
        let evidence_yes = prior_up * likelihood + (1.0 - prior_up) * (1.0 - likelihood);
        let bayes_up_prob = if evidence_yes > 0.0 {
            (prior_up * likelihood) / evidence_yes
        } else {
            0.50
        };

        // Blend Bayesian posterior with raw delta-based estimate
        let delta_estimate = 0.50 + (combined_delta / 0.30).clamp(-0.35, 0.35);
        let blend_weight = signal_strength * 0.5; // Max 50% weight to Bayesian
        let fair_up_prob = (bayes_up_prob * blend_weight + delta_estimate * (1.0 - blend_weight))
            .clamp(0.03, 0.97);

        // EV calculation using proper formula: EV = P × (1-cost) × (1-fee) - (1-P) × cost
        let polymarket_fee = 0.02;
        let cost_yes = ctx.yes_price;
        let cost_no = ctx.no_price;

        // Evaluate YES side
        let ev_yes = fair_up_prob * (1.0 - cost_yes) * (1.0 - polymarket_fee)
            - (1.0 - fair_up_prob) * cost_yes;

        // Evaluate NO side
        let fair_down_prob = 1.0 - fair_up_prob;
        let ev_no = fair_down_prob * (1.0 - cost_no) * (1.0 - polymarket_fee)
            - (1.0 - fair_down_prob) * cost_no;

        let min_edge = 0.05; // Minimum edge threshold

        if ev_yes > ev_no && ev_yes > min_edge && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            // Confidence from EV strength, mapped to [0.50, 0.85]
            let confidence = (0.50_f64 + ev_yes * 3.0).min(0.85_f64);
            Signal::Yes(confidence)
        } else if ev_no > ev_yes && ev_no > min_edge && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.50_f64 + ev_no * 3.0).min(0.85_f64);
            Signal::No(confidence)
        } else {
            Signal::Hold(format!("No Bayesian edge: EV_YES={:.4} EV_NO={:.4}", ev_yes, ev_no))
        }
    }

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

        let strong_threshold = self.params.min_delta * 1.5;

        if change > strong_threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            if ctx.yes_price > 0.60 {
                return Signal::Hold(format!("YES price too high: {:.0}c", ctx.yes_price * 100.0));
            }
            let confidence = (0.65_f64 + change * 150.0).min(0.88_f64);
            if confidence >= 0.70 {
                return Signal::Yes(confidence);
            }
            return Signal::Hold(format!("Confidence too low: {:.2}", confidence));
        }

        if change < -strong_threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            if ctx.no_price > 0.60 {
                return Signal::Hold(format!("NO price too high: {:.0}c", ctx.no_price * 100.0));
            }
            let confidence = (0.65_f64 + change.abs() * 150.0).min(0.88_f64);
            if confidence >= 0.70 {
                return Signal::No(confidence);
            }
            return Signal::Hold(format!("Confidence too low: {:.2}", confidence));
        }

        Signal::Hold(format!("No strong momentum: {:.4}%", change * 100.0))
    }

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

        let min_change = self.params.min_delta;

        if ctx.yes_price < 0.42 && ctx.yes_price >= 0.30 {
            if change > min_change && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
                let edge = 0.50 - ctx.yes_price;
                let confidence = (0.60_f64 + edge * 5.0 + change * 200.0).min(0.88_f64);
                return Signal::Yes(confidence);
            }
        }

        if ctx.no_price < 0.42 && ctx.no_price >= 0.30 {
            if change < -min_change && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
                let edge = 0.50 - ctx.no_price;
                let confidence = (0.60_f64 + edge * 5.0 + change.abs() * 200.0).min(0.88_f64);
                return Signal::No(confidence);
            }
        }

        if ctx.yes_price > 0.58 && ctx.yes_price <= 0.70 {
            if change < -min_change && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
                let confidence = (0.58_f64 + change.abs() * 200.0).min(0.80_f64);
                return Signal::No(confidence);
            }
        }

        Signal::Hold(format!(
            "No sniper arb: YES={:.0}c NO={:.0}c",
            ctx.yes_price * 100.0,
            ctx.no_price * 100.0
        ))
    }

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
        let min_vol = 0.0001;
        let max_vol = 0.0030;

        if abs_change < min_vol {
            return Signal::Hold(format!("Volatility too low: {:.4}%", abs_change * 100.0));
        }
        if abs_change > max_vol {
            return Signal::Hold(format!("Volatility too high: {:.4}%", abs_change * 100.0));
        }

        let confidence = 0.62_f64 + (abs_change / max_vol) * 0.20_f64;

        if change > 0.0 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            if ctx.yes_price > 0.65 {
                return Signal::Hold(format!("YES price too high: {:.0}c", ctx.yes_price * 100.0));
            }
            return Signal::Yes(confidence.min(0.82_f64));
        }

        if change < 0.0 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            if ctx.no_price > 0.65 {
                return Signal::Hold(format!("NO price too high: {:.0}c", ctx.no_price * 100.0));
            }
            return Signal::No(confidence.min(0.82_f64));
        }

        Signal::Hold(format!("No clear direction: {:.4}%", change * 100.0))
    }
}