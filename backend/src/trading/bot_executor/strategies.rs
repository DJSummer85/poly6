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

/// Strategy parameters with lower thresholds for more entries
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
            min_delta: 0.0005,       // 0.05% — érzékenyebb, hogy több szignál áttörjön
            min_price: 0.15,         // 15c — szélesebb sáv, több bemenet
            max_price: 0.85,         // 85c
            min_time_remaining: 10,  // 10 másodperc minimum
            max_time_remaining: 280, // 4 perc 40 másodperc maximum
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
            "velocity" => self.evaluate_velocity(ctx),
            "binance_velocity" => self.evaluate_binance_velocity(ctx),
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
            "edge_hunter" => self.evaluate_edge_hunter(ctx),
            "extreme_edge" => self.evaluate_extreme_edge(ctx),
            "yes_no_arb" => self.evaluate_yes_no_arb(ctx),
            "oracle_lag_v2" => self.evaluate_oracle_lag_v2(ctx),
            "low_volatility_edge" => self.evaluate_low_volatility_edge(ctx),
            "strict_momentum" => self.evaluate_strict_momentum(ctx),
            "patient_waiter" => self.evaluate_patient_waiter(ctx),
            "signal_momentum_v2" | "momentum_v2" => self.evaluate_signal_momentum_v2(ctx),
            _ => Signal::Hold(format!("Unknown strategy: {}", self.strategy_type)),
        }
    }

    fn check_price_limits(&self, action: &str, yes_price: f64, no_price: f64) -> bool {
        let target_price = if action == "YES" { yes_price } else { no_price };
        target_price >= self.params.min_price && target_price <= self.params.max_price
    }

    fn calculate_fair_prob(&self, delta_ratio: f64) -> f64 {
        0.5 + (delta_ratio / 0.05).tanh() * 0.45
    }

    fn has_sufficient_edge(&self, our_prob: f64, market_prob: f64, min_edge: f64) -> bool {
        (our_prob - market_prob).abs() >= min_edge
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

        // Strong signal: delta > 0.10% → higher confidence, more entries
        if delta_pct > 0.10 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.70_f64 + (delta_pct - 0.10) * 2.0).min(0.92_f64);
            return Signal::Yes(confidence);
        }
        if delta_pct < -0.10 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.70_f64 + (-delta_pct - 0.10) * 2.0).min(0.92_f64);
            return Signal::No(confidence);
        }
        // Medium signal: delta > 0.05% → good entry zone
        if delta_pct > 0.05 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.56_f64 + (delta_pct - 0.05) * 2.0).min(0.74_f64);
            return Signal::Yes(confidence);
        }
        if delta_pct < -0.05 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.56_f64 + (-delta_pct - 0.05) * 2.0).min(0.74_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("Delta too small: {:.4}%", delta_pct))
    }

    fn evaluate_oracle_lag(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too close to close".to_string());
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

        let threshold = self.params.min_delta;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = our_prob - market_prob;
            if edge >= 0.005 {
                let confidence = (0.60 + edge * 3.0).min(0.85);
                Signal::Yes(confidence)
            } else {
                Signal::Hold(format!("No oracle lag edge: our {:.1}% vs market {:.1}%", our_prob * 100.0, market_prob * 100.0))
            }
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = market_prob - our_prob;
            if edge >= 0.005 {
                let confidence = (0.60 + edge * 3.0).min(0.85);
                Signal::No(confidence)
            } else {
                Signal::Hold(format!("No oracle lag edge: our NO {:.1}% vs market NO {:.1}%", (1.0 - our_prob) * 100.0, (1.0 - market_prob) * 100.0))
            }
        } else {
            Signal::Hold(format!("No oracle lag: {:.4}%", change * 100.0))
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

        if delta_pct.abs() < 0.04 {
            return Signal::Hold(format!("Delta too small: {:.4}%", delta_pct));
        }

        let action = if delta_pct > 0.0 { "YES" } else { "NO" };
        let target_price = if action == "YES" { ctx.yes_price } else { ctx.no_price };

        if target_price > 0.80 {
            return Signal::Hold(format!("Price too high: {:.0}c", target_price * 100.0));
        }
        if target_price < 0.20 {
            return Signal::Hold(format!("Price too low: {:.0}c", target_price * 100.0));
        }

        let confidence = 0.58_f64 + (delta_pct.abs() * 3.0).min(0.25_f64);
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

        // Kiszámoljuk, hogy az indulás óta (window_open) merre tart az ár
        let window_delta = if let (Some(open), true) = (ctx.btc_window_open, ctx.btc_price > 0.0) {
            (ctx.btc_price - open) / open
        } else {
            0.0
        };

        // Overextended filter: if move is > 0.35%, it's likely to revert
        let overextended_threshold = 0.0035;
        let threshold = self.params.min_delta * 1.2;

        if change.abs() > overextended_threshold {
            return Signal::Hold(format!("Overextended: {:.3}%", change * 100.0));
        }

        // Csak akkor kötünk momentumra, ha a rövid távú lendület MEGEGYEZIK a piac indulása óta tartó fő iránnyal.
        let is_primary_trend_up = window_delta > -0.0005; // Megengedünk egy pici zajt
        let is_primary_trend_down = window_delta < 0.0005;

        if change > threshold && is_primary_trend_up && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = our_prob - market_prob;
            if edge >= 0.005 {
                let confidence = (0.55 + edge * 3.0).min(0.82);
                Signal::Yes(confidence)
            } else {
                Signal::Hold(format!("No edge: our {:.1}% vs market {:.1}%", our_prob * 100.0, market_prob * 100.0))
            }
        } else if change < -threshold && is_primary_trend_down && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = market_prob - our_prob;
            if edge >= 0.005 {
                let confidence = (0.55 + edge * 3.0).min(0.82);
                Signal::No(confidence)
            } else {
                Signal::Hold(format!("No edge: our NO {:.1}% vs market NO {:.1}%", (1.0 - our_prob) * 100.0, (1.0 - market_prob) * 100.0))
            }
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
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = our_prob - market_prob;
            if edge >= 0.005 {
                let confidence = (0.55 + edge * 3.0).min(0.80);
                Signal::Yes(confidence)
            } else {
                Signal::Hold(format!("No trend edge: our {:.1}% vs market {:.1}%", our_prob * 100.0, market_prob * 100.0))
            }
        } else if change < -threshold && is_primary_trend_down && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = market_prob - our_prob;
            if edge >= 0.005 {
                let confidence = (0.55 + edge * 3.0).min(0.80);
                Signal::No(confidence)
            } else {
                Signal::Hold(format!("No trend edge: our NO {:.1}% vs market NO {:.1}%", (1.0 - our_prob) * 100.0, (1.0 - market_prob) * 100.0))
            }
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

        let window_delta = if let (Some(open), true) = (ctx.btc_window_open, ctx.btc_price > 0.0) {
            (ctx.btc_price - open) / open
        } else {
            0.0
        };

        let threshold = self.params.min_delta * 1.5;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = our_prob - market_prob;
            if edge >= 0.005 {
                let confidence = (0.58 + edge * 3.0).min(0.82);
                Signal::Yes(confidence)
            } else {
                Signal::Hold(format!("No volatility edge: our {:.1}% vs market {:.1}%", our_prob * 100.0, market_prob * 100.0))
            }
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = market_prob - our_prob;
            if edge >= 0.005 {
                let confidence = (0.58 + edge * 3.0).min(0.82);
                Signal::No(confidence)
            } else {
                Signal::Hold(format!("No volatility edge: our NO {:.1}% vs market NO {:.1}%", (1.0 - our_prob) * 100.0, (1.0 - market_prob) * 100.0))
            }
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

        let window_delta = if let (Some(open), true) = (ctx.btc_window_open, ctx.btc_price > 0.0) {
            (ctx.btc_price - open) / open
        } else {
            0.0
        };

        let threshold = self.params.min_delta * 1.5;

        if change > threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = our_prob - market_prob;
            if edge >= 0.005 {
                let confidence = (0.60 + edge * 3.0).min(0.85);
                Signal::Yes(confidence)
            } else {
                Signal::Hold(format!("No sniper edge: our {:.1}% vs market {:.1}%", our_prob * 100.0, market_prob * 100.0))
            }
        } else if change < -threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = market_prob - our_prob;
            if edge >= 0.005 {
                let confidence = (0.60 + edge * 3.0).min(0.85);
                Signal::No(confidence)
            } else {
                Signal::Hold(format!("No sniper edge: our NO {:.1}% vs market NO {:.1}%", (1.0 - our_prob) * 100.0, (1.0 - market_prob) * 100.0))
            }
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

        let window_delta = if let (Some(open), true) = (ctx.btc_window_open, ctx.btc_price > 0.0) {
            (ctx.btc_price - open) / open
        } else {
            0.0
        };

        let threshold = self.params.min_delta * 1.5;

        if change > threshold && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = market_prob - our_prob; // Market YES is higher than our fair YES → buy NO
            if edge >= 0.005 {
                let confidence = (0.55 + edge * 3.0).min(0.75);
                Signal::No(confidence)
            } else {
                Signal::Hold(format!("No contrarian edge: market YES {:.1}% vs our {:.1}%", market_prob * 100.0, our_prob * 100.0))
            }
        } else if change < -threshold && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let our_prob = self.calculate_fair_prob(window_delta);
            let market_prob = ctx.yes_price;
            let edge = our_prob - market_prob; // Our fair YES is higher than market YES → buy YES
            if edge >= 0.02 {
                let confidence = (0.55 + edge * 3.0).min(0.75);
                Signal::Yes(confidence)
            } else {
                Signal::Hold(format!("No contrarian edge: our YES {:.1}% vs market YES {:.1}%", our_prob * 100.0, market_prob * 100.0))
            }
        } else {
            Signal::Hold("No contrarian signal".to_string())
        }
    }

    fn evaluate_mean_reversion(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late".to_string());
        }

        if ctx.yes_price > 0.72 {
            Signal::No(0.70)
        } else if ctx.yes_price < 0.28 {
            Signal::Yes(0.70)
        } else if ctx.yes_price > 0.62 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            Signal::No(0.60)
        } else if ctx.yes_price < 0.38 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            Signal::Yes(0.60)
        } else {
            Signal::Hold(format!("Price near fair value: {:.0}c", ctx.yes_price * 100.0))
        }
    }

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

        // Avoid extreme volatility only (raised from 0.003 to 0.006 - 0.003 was blocking normal BTC)
        if btc_volatility > 0.006 {
            return Signal::Hold("Extreme volatility - market unpredictable".to_string());
        }

        // Minimum velocity threshold
        let min_velocity: f64 = 0.00002;

        if velocity.abs() < min_velocity {
            return Signal::Hold(format!("Velocity too low: {:.4}%/s (choppy)", velocity * 100.0));
        }

        let is_up = velocity > 0.0;

        // Check price limits
        let action = if is_up { "YES" } else { "NO" };
        let target_price = if is_up { ctx.yes_price } else { ctx.no_price };

        if target_price < self.params.min_price || target_price > self.params.max_price {
            return Signal::Hold(format!("Price out of range: {:.0}c", target_price * 100.0));
        }

        // Confidence: velocity strength + acceleration boost (positive if accelerating, small penalty if decelerating)
        let vel_strength = (velocity.abs() * 800.0).min(0.25);
        let acc_in_direction = if is_up { acceleration } else { -acceleration };
        let acc_boost = (acc_in_direction * 800.0).clamp(-0.05, 0.15);
        let base_confidence = 0.55_f64;
        let confidence = (base_confidence + vel_strength + acc_boost).min(0.80);

        tracing::info!(
            "Velocity: {} | vel={:.4}%/s acc={:.5}%/s² conf={:.2}",
            action, velocity * 100.0, acceleration * 100.0, confidence
        );

        if is_up {
            Signal::Yes(confidence)
        } else {
            Signal::No(confidence)
        }
    }

    fn evaluate_binance_velocity(&self, ctx: StrategyContext) -> Signal {
    // Similar to evaluate_velocity but with stricter thresholds for Binance data
    if ctx.time_remaining < 45 {
        return Signal::Hold("Too close to closure".to_string());
    }

    let velocity = ctx.btc_velocity.unwrap_or(0.0);
    let acceleration = ctx.btc_acceleration.unwrap_or(0.0);
    let btc_volatility = ctx.btc_volatility.unwrap_or(0.0);

    if ctx.btc_price == 0.0 {
        return Signal::Hold("No BTC price".to_string());
    }

    // Avoid extreme volatility only (raised from 0.003 to 0.006)
    if btc_volatility > 0.006 {
        return Signal::Hold("Extreme volatility - market unpredictable".to_string());
    }

    // Stricter velocity threshold for Binance data
    let min_velocity: f64 = 0.00003; // 0.003%/s

    if velocity.abs() < min_velocity {
        return Signal::Hold(format!("Velocity too low: {:.4}%/s (choppy)", velocity * 100.0));
    }

    let is_up = velocity > 0.0;

    // Check price limits
    let action = if is_up { "YES" } else { "NO" };
    let target_price = if is_up { ctx.yes_price } else { ctx.no_price };

    if target_price < self.params.min_price || target_price > self.params.max_price {
        return Signal::Hold(format!("Price out of range: {:.0}c", target_price * 100.0));
    }

    // Confidence: velocity strength + acceleration boost (positive if accelerating, small penalty if decelerating)
    let vel_strength = (velocity.abs() * 1000.0).min(0.30);
    let acc_in_direction = if is_up { acceleration } else { -acceleration };
    let acc_boost = (acc_in_direction * 1000.0).clamp(-0.05, 0.20);
    let base_confidence = 0.60_f64;
    let confidence = (base_confidence + vel_strength + acc_boost).min(0.85);

    tracing::info!(
        "Binance Velocity: {} | vel={:.4}%/s acc={:.5}%/s² conf={:.2}",
        action, velocity * 100.0, acceleration * 100.0, confidence
    );

    if is_up {
        Signal::Yes(confidence)
    } else {
        Signal::No(confidence)
    }
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

        // YES overpriced + BTC falling → bet NO (check NO price limits, not YES!)
        if ctx.yes_price > 0.55 && ctx.no_price >= self.params.min_price && (change < -min_change || window_delta < -self.params.min_delta) {
            let conf = if both_confirm { 0.68 } else { 0.60 };
            Signal::No(conf)
        } else if ctx.yes_price < 0.45 && ctx.yes_price >= self.params.min_price && (change > min_change || window_delta > self.params.min_delta) {
            let conf = if both_confirm { 0.68 } else { 0.60 };
            Signal::Yes(conf)
        // Extrém árfekvés: gyengébb BTC megerősítés is elég
        } else if ctx.yes_price > 0.62 && ctx.no_price >= self.params.min_price && change < -min_change * 0.5 {
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

        if ctx.yes_price > 0.78 {
            Signal::No(0.68)
        } else if ctx.yes_price < 0.22 {
            Signal::Yes(0.68)
        } else if ctx.yes_price > 0.65 {
            Signal::No(0.60)
        } else if ctx.yes_price < 0.35 {
            Signal::Yes(0.60)
        } else if ctx.yes_price > 0.58 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            Signal::No(0.54)
        } else if ctx.yes_price < 0.42 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            Signal::Yes(0.54)
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

        if delta_pct.abs() < 0.03 {
            return Signal::Hold("Delta too small".to_string());
        }

        if delta_pct > 0.0 && self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + delta_pct.abs() * 2.5).min(0.82_f64);
            Signal::Yes(confidence)
        } else if delta_pct < 0.0 && self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
            let confidence = (0.55_f64 + delta_pct.abs() * 2.5).min(0.82_f64);
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

        if ctx.yes_price < 0.40 && ctx.yes_price >= 0.05 && (delta_pct > 0.04 || change > min_change) {
            let value_depth = (0.40 - ctx.yes_price) / 0.35;
            let confidence = (0.55_f64 + value_depth * 0.22).min(0.85_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.40 && ctx.no_price >= 0.05 && (delta_pct < -0.04 || change < -min_change) {
            let value_depth = (0.40 - ctx.no_price) / 0.35;
            let confidence = (0.55_f64 + value_depth * 0.22).min(0.85_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("Not in ultra-low range: YES={:.0}c NO={:.0}c", ctx.yes_price * 100.0, ctx.no_price * 100.0))
    }

    fn evaluate_sniper_value(&self, ctx: StrategyContext) -> Signal {
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

        if ctx.yes_price < 0.40 && ctx.yes_price >= 0.05 && (delta_pct > 0.04 || change > min_change) {
            let value_depth = (0.40 - ctx.yes_price) / 0.35;
            let confidence = (0.60_f64 + value_depth * 0.22).min(0.90_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.40 && ctx.no_price >= 0.05 && (delta_pct < -0.04 || change < -min_change) {
            let value_depth = (0.40 - ctx.no_price) / 0.35;
            let confidence = (0.60_f64 + value_depth * 0.22).min(0.90_f64);
            return Signal::No(confidence);
        }

        Signal::Hold(format!("No sniper setup: YES={:.0}c NO={:.0}c", ctx.yes_price * 100.0, ctx.no_price * 100.0))
    }

    fn evaluate_odds_swing(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }

        let change = ctx.btc_change.unwrap_or(0.0);
        let min_change = self.params.min_delta * 0.5;

        if ctx.yes_price < 0.30 && ctx.yes_price >= 0.04 && change > min_change {
            let confidence = (0.55_f64 + (0.30 - ctx.yes_price) * 2.0).min(0.80_f64);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.30 && ctx.no_price >= 0.04 && change < -min_change {
            let confidence = (0.55_f64 + (0.30 - ctx.no_price) * 2.0).min(0.80_f64);
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

    fn evaluate_edge_hunter(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < self.params.min_time_remaining {
            return Signal::Hold("Too late to trade".to_string());
        }
        if ctx.time_remaining > self.params.max_time_remaining {
            return Signal::Hold("Window just started".to_string());
        }
        if ctx.btc_price == 0.0 {
            return Signal::Hold("No BTC price".to_string());
        }

        let window_open = match ctx.btc_window_open {
            Some(open) if open > 0.0 => open,
            _ => return Signal::Hold("No BTC window data".to_string()),
        };

        // Calculate BTC delta ratio and percentage
        let delta_ratio = (ctx.btc_price - window_open) / window_open;
        let delta_pct = delta_ratio * 100.0;

        // Need minimum BTC movement
        if delta_ratio.abs() < self.params.min_delta {
            return Signal::Hold(format!(
                "BTC delta {:.3}% < threshold {:.3}%",
                delta_pct,
                self.params.min_delta * 100.0
            ));
        }

        // Calculate our fair probability from delta ratio using tanh calibration helper
        let our_prob = self.calculate_fair_prob(delta_ratio);

        // Get market implied probability (YES price = probability)
        let market_prob = ctx.yes_price;

        // Calculate edge: positive means we think it's more likely than market
        let edge = our_prob - market_prob;
        let min_edge = 0.005; // 0.5% minimum edge

        // Only trade if we have sufficient edge (our_prob vs market_prob)
        if self.has_sufficient_edge(our_prob, market_prob, min_edge) {
            if edge > 0.0 {
                // Our probability > market - market is undervaluing YES
                if self.check_price_limits("YES", ctx.yes_price, ctx.no_price) {
                    let confidence = (0.55 + edge * 3.0).min(0.82);
                    return Signal::Yes(confidence);
                } else {
                    return Signal::Hold("YES price outside limits".to_string());
                }
            } else {
                // Market thinks this is MORE likely than we do - bet against
                if self.check_price_limits("NO", ctx.yes_price, ctx.no_price) {
                    let confidence = (0.55 + (-edge) * 3.0).min(0.82);
                    return Signal::No(confidence);
                } else {
                    return Signal::Hold("NO price outside limits".to_string());
                }
            }
        }

        Signal::Hold(format!(
            "No edge: our {:.1}% vs market {:.1}% (need {:.1}% edge)",
            our_prob * 100.0,
            market_prob * 100.0,
            min_edge * 100.0
        ))
    }

    fn evaluate_extreme_edge(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 20 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 270 {
            return Signal::Hold("Window just opened".to_string());
        }

        let btc_move = ctx.btc_change.map(|c| c.abs()).unwrap_or(0.0);

        if ctx.yes_price > 0.65 {
            let edge = ctx.yes_price - 0.50;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: YES overpriced at {:.0}c, betting NO conf={:.2}", ctx.yes_price * 100.0, confidence);
            return Signal::No(confidence);
        }

        if ctx.yes_price < 0.35 {
            let edge = 0.50 - ctx.yes_price;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: YES underpriced at {:.0}c, betting YES conf={:.2}", ctx.yes_price * 100.0, confidence);
            return Signal::Yes(confidence);
        }

        if ctx.no_price > 0.65 {
            let edge = ctx.no_price - 0.50;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: NO overpriced at {:.0}c, betting YES conf={:.2}", ctx.no_price * 100.0, confidence);
            return Signal::Yes(confidence);
        }

        if ctx.no_price < 0.35 {
            let edge = 0.50 - ctx.no_price;
            let confidence = (0.60_f64 + edge * 3.0).min(0.88_f64);
            tracing::info!("Extreme Edge: NO underpriced at {:.0}c, betting NO conf={:.2}", ctx.no_price * 100.0, confidence);
            return Signal::No(confidence);
        }

        if ctx.yes_price > 0.58 {
            let edge = ctx.yes_price - 0.50;
            let confidence = (0.52_f64 + edge * 2.0).min(0.70_f64);
            if btc_move > 0.0002 {
                return Signal::No(confidence);
            }
            return Signal::Hold(format!("Slight edge but BTC flat: {:.4}%", btc_move * 100.0));
        }

        if ctx.yes_price < 0.42 {
            let edge = 0.50 - ctx.yes_price;
            let confidence = (0.52_f64 + edge * 2.0).min(0.70_f64);
            if btc_move > 0.0002 {
                return Signal::Yes(confidence);
            }
            return Signal::Hold(format!("Slight edge but BTC flat: {:.4}%", btc_move * 100.0));
        }

        Signal::Hold(format!(
            "No extreme edge: YES={:.0}c NO={:.0}c",
            ctx.yes_price * 100.0,
            ctx.no_price * 100.0
        ))
    }

    fn evaluate_yes_no_arb(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 280 {
            return Signal::Hold("Window just opened".to_string());
        }

        let combined = ctx.yes_price + ctx.no_price;

        if combined < 0.95 {
            tracing::info!(
                "YES_NO_ARB STRONG: YES={:.0}c + NO={:.0}c = {:.2} < $0.95 → Guaranteed profit {:.2}/share",
                ctx.yes_price * 100.0,
                ctx.no_price * 100.0,
                combined,
                1.0 - combined
            );
            let confidence = (0.60_f64 + (0.95 - combined) * 8.0).min(0.92_f64);
            return Signal::Yes(confidence);
        }

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

        let min_change = 0.03_f64;
        let ideal_min = 0.05_f64;
        let ideal_max = 0.20_f64;

        if abs_change < min_change {
            return Signal::Hold(format!("Change too small: {:.4}% (need >{:.2}%)", abs_change, min_change));
        }

        if abs_change >= ideal_min && abs_change <= ideal_max {
            let confidence = if abs_change >= 0.10 {
                (0.68_f64 + abs_change * 1.5).min(0.88_f64)
            } else {
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

    fn evaluate_low_volatility_edge(&self, ctx: StrategyContext) -> Signal {
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

        let noise_threshold = 0.01_f64;
        let min_signal = 0.02_f64;
        let strong_signal = 0.06_f64;

        if abs_change < noise_threshold {
            return Signal::Hold(format!("Price noise: {:.4}%", abs_change));
        }

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

        let mut confidence = 0.55_f64;

        if abs_change >= strong_signal {
            confidence += 0.18_f64;
        } else if abs_change >= 0.04_f64 {
            confidence += 0.12_f64;
        } else if abs_change >= min_signal {
            confidence += 0.06_f64;
        }

        if yes_edge >= 0.08_f64 || no_edge >= 0.08_f64 {
            confidence += 0.10_f64;
        } else if yes_edge >= 0.05_f64 || no_edge >= 0.05_f64 {
            confidence += 0.06_f64;
        }

        let mid_window = 150_f64;
        let time_from_mid = (ctx.time_remaining as f64 - mid_window).abs();
        if time_from_mid < 60_f64 {
            confidence += 0.05_f64;
        }

        confidence = confidence.min(0.82_f64);

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

    fn evaluate_strict_momentum(&self, ctx: StrategyContext) -> Signal {
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

        let threshold = 0.15_f64;

        if abs_change < threshold {
            return Signal::Hold(format!(
                "Delta {:.3}% < {:.2}% threshold (noise)",
                abs_change, threshold
            ));
        }

        if ctx.yes_price < 0.35 || ctx.yes_price > 0.65 {
            return Signal::Hold(format!(
                "PM price {:.1}% outside [35-65%] range",
                ctx.yes_price * 100.0
            ));
        }

        if change > 0.0 {
            let confidence = (0.65_f64 + abs_change * 1.5).min(0.88_f64);
            tracing::info!(
                "STRICT_MOMENTUM: BTC +{:.3}% → YES conf={:.2}",
                change_pct, confidence
            );
            Signal::Yes(confidence)
        } else {
            let confidence = (0.65_f64 + abs_change * 1.5).min(0.88_f64);
            tracing::info!(
                "STRICT_MOMENTUM: BTC {:.3}% → NO conf={:.2}",
                change_pct, confidence
            );
            Signal::No(confidence)
        }
    }

    fn evaluate_patient_waiter(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 270 {
            return Signal::Hold("Window just opened".to_string());
        }

        let distance_from_50 = (ctx.yes_price - 0.5).abs();
        let max_distance = 0.05_f64;

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

        let min_delta = 0.12_f64;
        if delta_pct.abs() < min_delta {
            return Signal::Hold(format!(
                "BTC delta {:.3}% < {:.2}% (no clear direction)",
                delta_pct, min_delta
            ));
        }

        if delta_pct > 0.0 {
            let confidence = (0.60_f64 + delta_pct * 2.0).min(0.85_f64);
            tracing::info!(
                "PATIENT_WAITER: BTC +{:.3}%, YES@{:.1}% → YES conf={:.2}",
                delta_pct,
                ctx.yes_price * 100.0,
                confidence
            );
            Signal::Yes(confidence)
        } else {
            let confidence = (0.60_f64 + (-delta_pct) * 2.0).min(0.85_f64);
            tracing::info!(
                "PATIENT_WAITER: BTC {:.3}%, NO@{:.1}% → NO conf={:.2}",
                delta_pct,
                ctx.no_price * 100.0,
                confidence
            );
            Signal::No(confidence)
        }
    }

    fn evaluate_signal_momentum_v2(&self, ctx: StrategyContext) -> Signal {
        if ctx.time_remaining < 30 {
            return Signal::Hold("Too close to close".to_string());
        }
        if ctx.time_remaining > 250 {
            return Signal::Hold("Window just opened".to_string());
        }

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

        let min_delta = 0.20_f64;
        if abs_delta < min_delta {
            return Signal::Hold(format!(
                "BTC delta {:.3}% < {:.2}% threshold (noise)",
                delta_pct, min_delta
            ));
        }

        let time_factor = if ctx.time_remaining > 150 {
            1.0
        } else if ctx.time_remaining > 60 {
            0.95
        } else {
            0.90
        };

        let base_confidence = 0.55_f64;
        let max_confidence = 0.75_f64;
        let delta_strength = (abs_delta - min_delta).min(0.30);
        let raw_confidence =
            base_confidence + (delta_strength / 0.30) * (max_confidence - base_confidence);
        let confidence = (raw_confidence * time_factor).min(max_confidence);

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