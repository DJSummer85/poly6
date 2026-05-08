//! Risk Manager - Per-bot and portfolio-level risk controls
//! Ported from polymarket-demo/src/lib/risk-manager.ts

use std::collections::HashMap;
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct RiskSettings {
    pub max_daily_loss: f64,
    pub max_position_size: f64,
    pub max_drawdown_percent: f64,
    pub min_confidence: f64,
    pub cooldown_after_loss_secs: f64,
    pub max_trades_per_hour: u32,
    pub portfolio_max_loss: f64,
    pub portfolio_max_drawdown: f64,
    pub kelly_enabled: bool,
    pub kelly_fraction: f64,
    pub kelly_min_confidence: f64,
    pub circuit_breaker_enabled: bool,
    pub consecutive_loss_threshold: u32,
    pub auto_reduce_on_loss: bool,
    pub auto_increase_on_win: bool,
}

impl Default for RiskSettings {
    fn default() -> Self {
        Self {
            max_daily_loss: 5.0,
            max_position_size: 3.0,
            max_drawdown_percent: 20.0,
            min_confidence: 0.55,
            cooldown_after_loss_secs: 15.0,
            max_trades_per_hour: 40,
            portfolio_max_loss: 10.0,
            portfolio_max_drawdown: 25.0,
            kelly_enabled: true,
            kelly_fraction: 0.25,
            kelly_min_confidence: 0.55,
            circuit_breaker_enabled: true,
            consecutive_loss_threshold: 5,
            auto_reduce_on_loss: true,
            auto_increase_on_win: true,
        }
    }
}

impl RiskSettings {
    /// Relaxed settings for paper/demo trading
    pub fn paper_mode() -> Self {
        Self {
            max_daily_loss: 20.0,
            max_drawdown_percent: 50.0,
            min_confidence: 0.45,
            cooldown_after_loss_secs: 5.0,
            consecutive_loss_threshold: 15,
            circuit_breaker_enabled: false,
            portfolio_max_loss: 30.0,
            portfolio_max_drawdown: 60.0,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct BotRiskState {
    pub daily_pnl: f64,
    pub daily_trades: u32,
    pub current_drawdown: f64,
    pub last_loss_time: Option<i64>,
    pub trades_this_hour: u32,
    pub hour_start: i64,
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub consecutive_wins: u32,
    pub consecutive_losses: u32,
    pub last_trade_result: Option<String>,
    pub current_bet_multiplier: f64,
}

impl Default for BotRiskState {
    fn default() -> Self {
        Self {
            daily_pnl: 0.0,
            daily_trades: 0,
            current_drawdown: 0.0,
            last_loss_time: None,
            trades_this_hour: 0,
            hour_start: Utc::now().timestamp(),
            paused: false,
            pause_reason: None,
            consecutive_wins: 0,
            consecutive_losses: 0,
            last_trade_result: None,
            current_bet_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RiskWarning {
    pub bot_id: i64,
    pub warning_type: String,
    pub message: String,
    pub severity: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct RiskStatus {
    pub current_drawdown: f64,
    pub daily_pnl: f64,
    pub trades_today: u32,
    pub warnings: Vec<RiskWarning>,
    pub actions: Vec<String>,
    pub paused: bool,
    pub pause_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub longest_win_streak: u32,
    pub longest_loss_streak: u32,
}

pub struct RiskManager {
    settings: RiskSettings,
    bot_states: HashMap<i64, BotRiskState>,
    warnings: Vec<RiskWarning>,
    last_reset_date: String,
    portfolio_start_balance: f64,
}

impl RiskManager {
    pub fn new(settings: RiskSettings) -> Self {
        Self {
            settings,
            bot_states: HashMap::new(),
            warnings: Vec::new(),
            last_reset_date: Self::today_date(),
            portfolio_start_balance: 100.0,
        }
    }

    pub fn new_paper() -> Self {
        Self::new(RiskSettings::paper_mode())
    }

    pub fn new_live() -> Self {
        Self::new(RiskSettings::default())
    }

    pub fn get_settings(&self) -> &RiskSettings {
        &self.settings
    }

    pub fn update_settings(&mut self, f: impl FnOnce(&mut RiskSettings)) {
        f(&mut self.settings);
    }

    fn today_date() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    fn init_bot(&mut self, bot_id: i64) {
        self.bot_states.entry(bot_id).or_default();
    }

    /// Calculate position size using Kelly Criterion
    /// f* = (bp - q) / b
    /// b = (1 - price) / price (net odds)
    /// p = confidence (win probability)
    pub fn calculate_kelly_size(
        &self,
        confidence: f64,
        price: f64,
        bankroll: f64,
        kelly_fraction: f64,
    ) -> f64 {
        if confidence < self.settings.kelly_min_confidence {
            return 0.0;
        }

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

        kelly *= kelly_fraction;
        kelly = kelly.min(0.25); // Cap at 25% of bankroll

        bankroll * kelly
    }

    /// Get suggested bet size combining Kelly and streak adjustments
    pub fn get_suggested_bet_size(
        &mut self,
        bot_id: i64,
        confidence: f64,
        price: f64,
        bankroll: f64,
    ) -> (f64, String) {
        self.init_bot(bot_id);
        let state = self.bot_states.get(&bot_id).unwrap().clone();

        let mut size = bankroll * 0.1;
        let mut method = String::from("default_10pct");

        if self.settings.kelly_enabled && confidence >= self.settings.kelly_min_confidence {
            let kelly_size = self.calculate_kelly_size(confidence, price, bankroll, self.settings.kelly_fraction);
            if kelly_size > 0.0 {
                size = kelly_size;
                method = String::from("kelly");
            }
        }

        // Streak adjustments
        if self.settings.auto_reduce_on_loss && state.consecutive_losses > 0 {
            let reduction = 0.5_f64.powi(state.consecutive_losses.min(3) as i32);
            size *= reduction;
            method.push_str("_reduced");
        }

        if self.settings.auto_increase_on_win && state.consecutive_wins >= 3 {
            let increase = (1.0 + (state.consecutive_wins - 3) as f64 * 0.25).min(2.0);
            size *= increase;
            method.push_str("_increased");
        }

        size = size.min(self.settings.max_position_size).max(0.1);

        (size, method)
    }

    /// Record a trade result for auto-adjustment tracking
    pub fn record_trade_result(&mut self, bot_id: i64, won: bool) {
        self.init_bot(bot_id);
        let state = self.bot_states.get_mut(&bot_id).unwrap();

        if won {
            state.consecutive_wins += 1;
            state.consecutive_losses = 0;
            state.last_trade_result = Some("win".to_string());
        } else {
            state.consecutive_losses += 1;
            state.consecutive_wins = 0;
            state.last_trade_result = Some("loss".to_string());
            state.last_loss_time = Some(Utc::now().timestamp());
        }

        state.daily_trades += 1;
        state.daily_pnl += if won { 1.0 } else { -1.0 };

        // Circuit breaker
        if state.consecutive_losses >= self.settings.consecutive_loss_threshold
            && self.settings.circuit_breaker_enabled
        {
            let reason = format!("Circuit breaker: {} consecutive losses", state.consecutive_losses);
            state.paused = true;
            state.pause_reason = Some(reason.clone());
            self.add_warning(bot_id, "circuit_breaker", reason, "critical");
        }
    }

    /// Check if a new position can be opened
    pub fn can_open_position(
        &mut self,
        bot_id: i64,
        amount: f64,
        confidence: f64,
        current_balance: f64,
        initial_balance: f64,
    ) -> (bool, Option<String>) {
        self.check_daily_reset();
        self.init_bot(bot_id);

        let state = self.bot_states.get(&bot_id).unwrap().clone();

        // Paused?
        if state.paused {
            return (false, state.pause_reason);
        }

        // Position size
        if amount > self.settings.max_position_size {
            let msg = format!("Max position size is ${:.2}", self.settings.max_position_size);
            self.add_warning(bot_id, "position_size", format!("${:.2} exceeds max", amount), "warning");
            return (false, Some(msg));
        }

        // Confidence
        if confidence < self.settings.min_confidence {
            let msg = format!("Confidence {:.0}% below minimum {:.0}%",
                confidence * 100.0, self.settings.min_confidence * 100.0);
            return (false, Some(msg));
        }

        // Cooldown after loss
        if let Some(last_loss) = state.last_loss_time {
            if self.settings.cooldown_after_loss_secs > 0.0 {
                let elapsed = (Utc::now().timestamp() - last_loss) as f64;
                if elapsed < self.settings.cooldown_after_loss_secs {
                    let wait = (self.settings.cooldown_after_loss_secs - elapsed).ceil() as i64;
                    let msg = format!("Cooldown: wait {}s after loss", wait);
                    return (false, Some(msg));
                }
            }
        }

        // Rate limit
        self.check_hour_reset(bot_id);
        let state = self.bot_states.get(&bot_id).unwrap();
        if state.trades_this_hour >= self.settings.max_trades_per_hour {
            let msg = format!("Rate limit: max {} trades/hour", self.settings.max_trades_per_hour);
            self.add_warning(bot_id, "rate_limit", msg.clone(), "warning");
            return (false, Some(msg));
        }

        // Daily loss
        let state = self.bot_states.get(&bot_id).unwrap();
        if state.daily_pnl < -self.settings.max_daily_loss {
            let msg = format!("Daily loss limit reached (${:.2})", self.settings.max_daily_loss);
            self.add_warning(bot_id, "daily_loss", msg.clone(), "critical");
            return (false, Some(msg));
        }

        // Portfolio-level drawdown
        let portfolio_pnl = current_balance - self.portfolio_start_balance;
        if portfolio_pnl < -self.settings.portfolio_max_loss {
            let msg = format!("Portfolio loss limit reached (${:.2})", self.settings.portfolio_max_loss);
            self.add_warning(bot_id, "portfolio_loss", msg.clone(), "critical");
            return (false, Some(msg));
        }

        let drawdown = if self.portfolio_start_balance > 0.0 {
            ((self.portfolio_start_balance - current_balance) / self.portfolio_start_balance) * 100.0
        } else {
            0.0
        };
        if drawdown > self.settings.portfolio_max_drawdown {
            let msg = format!("Portfolio drawdown {:.1}% exceeds max {:.0}%", drawdown, self.settings.portfolio_max_drawdown);
            self.add_warning(bot_id, "portfolio_loss", msg.clone(), "critical");
            return (false, Some(msg));
        }

        // Bot-level drawdown
        let bot_drawdown = if initial_balance > 0.0 {
            ((initial_balance - current_balance) / initial_balance) * 100.0
        } else {
            0.0
        };
        if bot_drawdown >= self.settings.max_drawdown_percent {
            let msg = format!("Drawdown limit reached: {:.1}%", bot_drawdown);
            self.add_warning(bot_id, "drawdown", msg.clone(), "critical");
            return (false, Some(msg));
        }

        (true, None)
    }

    /// Get adjusted bet size based on win/loss streaks
    pub fn get_adjusted_bet_size(&mut self, bot_id: i64, base_bet: f64) -> f64 {
        self.init_bot(bot_id);
        let state = self.bot_states.get(&bot_id).unwrap().clone();

        let mut multiplier = 1.0;

        if self.settings.auto_reduce_on_loss && state.consecutive_losses > 0 {
            let reduction = 0.5_f64.powi(state.consecutive_losses.min(3) as i32);
            multiplier *= reduction;
        }

        if self.settings.auto_increase_on_win && state.consecutive_wins >= 3 {
            let increase = (1.0 + (state.consecutive_wins - 3) as f64 * 0.25).min(2.0);
            multiplier *= increase;
        }

        let adjusted = base_bet * multiplier;
        (adjusted.max(base_bet * 0.25)).min(base_bet * 2.0)
    }

    /// Pause a bot
    pub fn pause_bot(&mut self, bot_id: i64, reason: String) {
        self.init_bot(bot_id);
        let state = self.bot_states.get_mut(&bot_id).unwrap();
        state.paused = true;
        state.pause_reason = Some(reason.clone());
        self.add_warning(bot_id, "drawdown", reason, "critical");
    }

    /// Resume a bot
    pub fn resume_bot(&mut self, bot_id: i64) {
        self.init_bot(bot_id);
        let state = self.bot_states.get_mut(&bot_id).unwrap();
        state.paused = false;
        state.pause_reason = None;
    }

    /// Get bot risk status
    pub fn get_bot_risk_status(&mut self, bot_id: i64, current_balance: f64, initial_balance: f64) -> RiskStatus {
        self.check_daily_reset();
        self.init_bot(bot_id);

        let state = self.bot_states.get(&bot_id).unwrap().clone();
        let drawdown = if initial_balance > 0.0 {
            ((initial_balance - current_balance) / initial_balance) * 100.0
        } else {
            0.0
        };

        let mut actions = Vec::new();
        if state.daily_pnl < -self.settings.max_daily_loss * 0.8 {
            actions.push("reduce_size".to_string());
        }
        if state.paused || drawdown >= self.settings.max_drawdown_percent * 0.8 {
            actions.push("stop_trading".to_string());
        }

        let bot_warnings: Vec<RiskWarning> = self.warnings
            .iter()
            .filter(|w| w.bot_id == bot_id)
            .take(10)
            .cloned()
            .collect();

        RiskStatus {
            current_drawdown: drawdown,
            daily_pnl: state.daily_pnl,
            trades_today: state.daily_trades,
            warnings: bot_warnings,
            actions,
            paused: state.paused,
            pause_reason: state.pause_reason,
        }
    }

    /// Get all warnings
    pub fn get_warnings(&self, limit: usize) -> Vec<RiskWarning> {
        self.warnings.iter().take(limit).cloned().collect()
    }

    /// Clear warnings for a bot
    pub fn clear_warnings(&mut self, bot_id: i64) {
        self.warnings.retain(|w| w.bot_id != bot_id);
    }

    /// Reset bot state
    pub fn reset_bot(&mut self, bot_id: i64) {
        self.bot_states.remove(&bot_id);
        self.clear_warnings(bot_id);
    }

    /// Set portfolio start balance
    pub fn set_portfolio_start_balance(&mut self, balance: f64) {
        self.portfolio_start_balance = balance;
    }

    // -- Private helpers --

    fn check_daily_reset(&mut self) {
        let today = Self::today_date();
        if today != self.last_reset_date {
            for state in self.bot_states.values_mut() {
                state.daily_pnl = 0.0;
                state.daily_trades = 0;
                state.trades_this_hour = 0;
                state.hour_start = Utc::now().timestamp();
            }
            self.last_reset_date = today;
        }
    }

    fn check_hour_reset(&mut self, bot_id: i64) {
        let now = Utc::now().timestamp();
        if let Some(state) = self.bot_states.get_mut(&bot_id) {
            if now - state.hour_start >= 3600 {
                state.trades_this_hour = 0;
                state.hour_start = now;
            }
        }
    }

    fn add_warning(&mut self, bot_id: i64, warning_type: impl Into<String>, message: impl Into<String>, severity: &str) {
        self.warnings.insert(0, RiskWarning {
            bot_id,
            warning_type: warning_type.into(),
            message: message.into(),
            severity: severity.to_string(),
            timestamp: Utc::now().timestamp_millis(),
        });
        if self.warnings.len() > 100 {
            self.warnings.truncate(100);
        }
    }
}

/// Risk Metrics Calculator - calculates Sharpe, profit factor, drawdown
pub struct RiskMetricsCalculator {
    balance_history: Vec<f64>,
    peak_balance: f64,
    trade_pnls: Vec<f64>,
    current_win_streak: u32,
    current_loss_streak: u32,
    longest_win_streak: u32,
    longest_loss_streak: u32,
}

impl RiskMetricsCalculator {
    pub fn new() -> Self {
        Self {
            balance_history: Vec::new(),
            peak_balance: 0.0,
            trade_pnls: Vec::new(),
            current_win_streak: 0,
            current_loss_streak: 0,
            longest_win_streak: 0,
            longest_loss_streak: 0,
        }
    }

    pub fn record_balance(&mut self, balance: f64) {
        self.balance_history.push(balance);
        if balance > self.peak_balance {
            self.peak_balance = balance;
        }
    }

    pub fn record_trade_pnl(&mut self, pnl: f64) {
        self.trade_pnls.push(pnl);
    }

    pub fn record_trade_result(&mut self, won: bool) {
        if won {
            self.current_win_streak += 1;
            self.current_loss_streak = 0;
            self.longest_win_streak = self.longest_win_streak.max(self.current_win_streak);
        } else {
            self.current_loss_streak += 1;
            self.current_win_streak = 0;
            self.longest_loss_streak = self.longest_loss_streak.max(self.current_loss_streak);
        }
    }

    pub fn calculate_max_drawdown(&self) -> f64 {
        if self.peak_balance == 0.0 || self.balance_history.is_empty() {
            return 0.0;
        }
        let mut max_dd: f64 = 0.0;
        for balance in &self.balance_history {
            let dd = (self.peak_balance - balance) / self.peak_balance * 100.0;
            max_dd = max_dd.max(dd);
        }
        max_dd
    }

    pub fn calculate_sharpe_ratio(&self) -> f64 {
        if self.trade_pnls.len() < 5 {
            return 0.0;
        }
        let avg = self.trade_pnls.iter().sum::<f64>() / self.trade_pnls.len() as f64;
        let variance = self.trade_pnls.iter().map(|p| (p - avg).powi(2)).sum::<f64>() / self.trade_pnls.len() as f64;
        let std_dev = variance.sqrt();
        if std_dev == 0.0 {
            return if avg > 0.0 { 999.0 } else { 0.0 };
        }
        avg / std_dev
    }

    pub fn calculate_profit_factor(&self) -> f64 {
        let gross_profit: f64 = self.trade_pnls.iter().filter(|p| **p > 0.0).sum();
        let gross_loss: f64 = self.trade_pnls.iter().filter(|p| **p < 0.0).map(|p| p.abs()).sum();
        if gross_loss == 0.0 {
            return if gross_profit > 0.0 { 999.0 } else { 0.0 };
        }
        gross_profit / gross_loss
    }

    pub fn get_metrics(&self) -> RiskMetrics {
        let wins: Vec<&f64> = self.trade_pnls.iter().filter(|p| **p > 0.0).collect();
        let losses: Vec<&f64> = self.trade_pnls.iter().filter(|p| **p < 0.0).collect();

        RiskMetrics {
            max_drawdown: self.calculate_max_drawdown(),
            sharpe_ratio: self.calculate_sharpe_ratio(),
            win_rate: if self.trade_pnls.is_empty() {
                0.0
            } else {
                wins.len() as f64 / self.trade_pnls.len() as f64
            },
            profit_factor: self.calculate_profit_factor(),
            avg_win: if wins.is_empty() { 0.0 } else { wins.iter().copied().sum::<f64>() / wins.len() as f64 },
            avg_loss: if losses.is_empty() { 0.0 } else { losses.iter().copied().map(|p| p.abs()).sum::<f64>() / losses.len() as f64 },
            longest_win_streak: self.longest_win_streak,
            longest_loss_streak: self.longest_loss_streak,
        }
    }

    pub fn reset(&mut self) {
        self.balance_history.clear();
        self.peak_balance = 0.0;
        self.trade_pnls.clear();
        self.current_win_streak = 0;
        self.current_loss_streak = 0;
        self.longest_win_streak = 0;
        self.longest_loss_streak = 0;
    }
}

impl Default for RiskMetricsCalculator {
    fn default() -> Self {
        Self::new()
    }
}
