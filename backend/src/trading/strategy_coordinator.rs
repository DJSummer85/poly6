//! Strategy Coordinator - Prevents conflicting trades between bots
//! Ported from polymarket-demo/src/lib/strategy-coordinator.ts

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct PendingDecision {
    pub bot_id: i64,
    pub bot_name: String,
    pub strategy: String,
    pub action: String, // "YES" or "NO"
    pub confidence: f64,
    pub bet_size: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct CoordinationResult {
    pub allowed: bool,
    pub reason: String,
    pub adjusted_bet_size: Option<f64>,
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    pub max_outcome_exposure: f64,
    pub conflict_mode: String, // "strict", "advisory", "first_wins"
    pub max_bots_same_outcome: u32,
    pub compatible_strategies: HashMap<String, Vec<String>>,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        let mut compatible: HashMap<String, Vec<String>> = HashMap::new();

        // Momentum strategies
        compatible.insert("window_delta".to_string(), vec!["momentum".into(), "binance_signal".into()]);
        compatible.insert("momentum".to_string(), vec!["window_delta".into(), "binance_signal".into()]);
        compatible.insert("binance_signal".to_string(), vec!["window_delta".into(), "momentum".into(), "last_seconds_scalp".into()]);
        compatible.insert("last_seconds_scalp".to_string(), vec!["binance_signal".into()]);

        // Trend
        compatible.insert("trend".to_string(), vec!["smart_trend".into()]);
        compatible.insert("smart_trend".to_string(), vec!["trend".into()]);

        // Counter-trend
        compatible.insert("mean_reversion".to_string(), vec!["contrarian".into()]);
        compatible.insert("contrarian".to_string(), vec!["mean_reversion".into()]);

        // Probabilistic
        compatible.insert("fair_value".to_string(), vec!["volatility_breakout".into()]);
        compatible.insert("volatility_breakout".to_string(), vec!["trend_pullback".into(), "binance_velocity".into()]);
        compatible.insert("volatility".to_string(), vec!["momentum".into()]);

        // Price extremes
        compatible.insert("ultra_low_entry".to_string(), vec!["price_reversion".into(), "sniper_value".into()]);
        compatible.insert("price_reversion".to_string(), vec!["ultra_low_entry".into()]);
        compatible.insert("sniper_value".to_string(), vec!["price_reversion".into()]);

        // Velocity
        compatible.insert("binance_velocity".to_string(), vec!["volatility_breakout".into(), "trend_pullback".into()]);

        // Trend pullback
        compatible.insert("trend_pullback".to_string(), vec!["volatility_breakout".into(), "binance_velocity".into()]);

        // Standalone
        compatible.insert("odds_swing".to_string(), vec![]);
        compatible.insert("bayesian_ev".to_string(), vec![]);

        Self {
            max_outcome_exposure: 0.4, // 40% max exposure
            conflict_mode: "strict".to_string(),
            max_bots_same_outcome: 2,
            compatible_strategies: compatible,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrategyCoordinator {
    config: CoordinatorConfig,
    pending_decisions: HashMap<String, PendingDecision>,
    recent_executions: HashMap<String, (String, u64)>, // key -> (outcome, timestamp)
    market_exposure: HashMap<String, (f64, f64)>,      // market -> (yes_exposure, no_exposure)
}

impl StrategyCoordinator {
    pub fn new(config: CoordinatorConfig) -> Self {
        Self {
            config,
            pending_decisions: HashMap::new(),
            recent_executions: HashMap::new(),
            market_exposure: HashMap::new(),
        }
    }

    pub fn default_with_config() -> Self {
        Self::new(CoordinatorConfig::default())
    }

    /// Register a pending decision. Returns whether the trade should proceed.
    #[allow(clippy::too_many_arguments)]
    pub fn register_decision(
        &mut self,
        market_id: &str,
        bot_id: i64,
        bot_name: &str,
        strategy: &str,
        action: &str,
        confidence: f64,
        bet_size: f64,
        total_portfolio_balance: f64,
    ) -> CoordinationResult {
        let now = Self::now_ms();
        let key = format!("{}-{}", market_id, bot_id);

        let pending = PendingDecision {
            bot_id,
            bot_name: bot_name.to_string(),
            strategy: strategy.to_string(),
            action: action.to_string(),
            confidence,
            bet_size,
            timestamp: now,
        };

        let mut warnings = Vec::new();

        // 1. Check for conflicts
        let conflict = self.check_for_conflicts(market_id, &pending);
        if !conflict.allowed {
            return conflict;
        }
        if let Some(w) = conflict.warnings {
            warnings.extend(w);
        }

        // 2. Check exposure
        let exposure = self.check_exposure(market_id, &pending, total_portfolio_balance);
        if !exposure.allowed {
            return exposure;
        }
        let mut final_bet = bet_size;
        if let Some(adjusted) = exposure.adjusted_bet_size {
            final_bet = adjusted;
            warnings.push(format!("Bet reduced to ${:.2} to limit exposure", adjusted));
        }

        // 3. Check outcome capacity
        let capacity = self.check_outcome_capacity(market_id, &pending);
        if !capacity.allowed {
            return capacity;
        }

        // Register
        self.pending_decisions.insert(key, pending);

        // Update effective bet for exposure tracking (pending)
        if let Some(adjusted) = exposure.adjusted_bet_size {
            final_bet = adjusted;
        }

        self.cleanup_stale(now);

        let adjusted = if Some(final_bet) != Some(bet_size) {
            Some(final_bet)
        } else {
            None
        };

        CoordinationResult {
            allowed: true,
            reason: "Trade approved".to_string(),
            adjusted_bet_size: adjusted,
            warnings: if warnings.is_empty() { None } else { Some(warnings) },
        }
    }

    /// Confirm a trade was executed
    pub fn confirm_execution(&mut self, market_id: &str, bot_id: i64, outcome: &str, amount: f64) {
        let key = format!("{}-{}", market_id, bot_id);
        self.pending_decisions.remove(&key);

        let now = Self::now_ms();
        self.recent_executions.insert(key, (outcome.to_string(), now));

        let exposure = self.market_exposure.entry(market_id.to_string()).or_insert((0.0, 0.0));
        if outcome == "YES" {
            exposure.0 += amount;
        } else {
            exposure.1 += amount;
        }
    }

    /// Cancel a pending decision
    pub fn cancel_decision(&mut self, market_id: &str, bot_id: i64) {
        let key = format!("{}-{}", market_id, bot_id);
        self.pending_decisions.remove(&key);
    }

    /// Reset state for a new market
    pub fn reset_market(&mut self, market_id: &str) {
        self.pending_decisions.retain(|k, _| !k.starts_with(market_id));
        self.recent_executions.retain(|k, _| !k.starts_with(market_id));
        self.market_exposure.remove(market_id);
    }

    /// Get current exposure for a market
    pub fn get_market_exposure(&self, market_id: &str) -> (f64, f64) {
        self.market_exposure.get(market_id).copied().unwrap_or((0.0, 0.0))
    }

    /// Update coordinator config
    pub fn update_config(&mut self, f: impl FnOnce(&mut CoordinatorConfig)) {
        f(&mut self.config);
    }

    /// Get config
    pub fn get_config(&self) -> &CoordinatorConfig {
        &self.config
    }

    // -- Private helpers --

    fn check_for_conflicts(&self, market_id: &str, decision: &PendingDecision) -> CoordinationResult {
        let opposite = if decision.action == "YES" { "NO" } else { "YES" };
        let mut warnings = Vec::new();

        // Check pending decisions from other bots
        for (key, pending) in &self.pending_decisions {
            if !key.starts_with(market_id) {
                continue;
            }
            if pending.bot_id == decision.bot_id {
                continue;
            }

            // Opposite position = conflict
            if pending.action == opposite {
                let msg = format!("Conflict: {} ({}) already pending {}",
                    pending.bot_name, pending.strategy, opposite);

                if self.config.conflict_mode == "strict" {
                    return CoordinationResult {
                        allowed: false,
                        reason: msg,
                        adjusted_bet_size: None,
                        warnings: None,
                    };
                } else if self.config.conflict_mode == "advisory" {
                    warnings.push(format!("Warning: {}", msg));
                }
            }

            // Same outcome, check compatibility
            if pending.action == decision.action {
                let compatible = self.config.compatible_strategies
                    .get(&decision.strategy)
                    .cloned()
                    .unwrap_or_default();
                if !compatible.contains(&pending.strategy) {
                    warnings.push(format!("Note: Similar strategies ({}, {}) on same outcome",
                        decision.strategy, pending.strategy));
                }
            }
        }

        // Check recent executions (within 2 seconds)
        let now = Self::now_ms();
        for (key, (outcome, ts)) in &self.recent_executions {
            if !key.starts_with(market_id) {
                continue;
            }
            if now - ts > 2000 {
                continue;
            }

            let exec_bot_id: i64 = key.replace(&format!("{}-", market_id), "")
                .parse().unwrap_or(-1);
            if exec_bot_id == decision.bot_id {
                continue;
            }

            if outcome == opposite {
                let msg = "Conflict: Bot recently executed opposite outcome".to_string();
                if self.config.conflict_mode == "strict" {
                    return CoordinationResult {
                        allowed: false,
                        reason: msg,
                        adjusted_bet_size: None,
                        warnings: None,
                    };
                } else if self.config.conflict_mode == "advisory" {
                    warnings.push(format!("Warning: {}", msg));
                }
            }
        }

        CoordinationResult {
            allowed: true,
            reason: if warnings.is_empty() { "No conflicts".to_string() } else { "Conflicts detected but allowed".to_string() },
            adjusted_bet_size: None,
            warnings: if warnings.is_empty() { None } else { Some(warnings) },
        }
    }

    fn check_exposure(
        &self,
        market_id: &str,
        decision: &PendingDecision,
        total_balance: f64,
    ) -> CoordinationResult {
        let (yes_exp, no_exp) = self.market_exposure.get(market_id).copied().unwrap_or((0.0, 0.0));

        // Add pending decisions
        let (mut pending_yes, mut pending_no) = (0.0, 0.0);
        for (key, pending) in &self.pending_decisions {
            if !key.starts_with(market_id) || pending.bot_id == decision.bot_id {
                continue;
            }
            if pending.action == "YES" {
                pending_yes += pending.bet_size;
            } else {
                pending_no += pending.bet_size;
            }
        }

        let outcome_key = if decision.action == "YES" { 0 } else { 1 };
        let current_exp = if outcome_key == 0 { yes_exp + pending_yes } else { no_exp + pending_no };
        let new_exp = current_exp + decision.bet_size;
        let exposure_fraction = if total_balance > 0.0 { new_exp / total_balance } else { 0.0 };

        if exposure_fraction > self.config.max_outcome_exposure {
            let max_allowed = total_balance * self.config.max_outcome_exposure - current_exp;
            if max_allowed < 0.1 {
                return CoordinationResult {
                    allowed: false,
                    reason: format!("Max exposure reached for {} ({:.1}% > {:.0}%)",
                        decision.action, exposure_fraction * 100.0, self.config.max_outcome_exposure * 100.0),
                    adjusted_bet_size: None,
                    warnings: None,
                };
            }
            return CoordinationResult {
                allowed: true,
                reason: "Bet size reduced to limit exposure".to_string(),
                adjusted_bet_size: Some(max_allowed.max(0.1)),
                warnings: None,
            };
        }

        CoordinationResult {
            allowed: true,
            reason: "Exposure within limits".to_string(),
            adjusted_bet_size: None,
            warnings: None,
        }
    }

    fn check_outcome_capacity(&self, market_id: &str, decision: &PendingDecision) -> CoordinationResult {
        let mut same_outcome_count = 0;

        for (key, pending) in &self.pending_decisions {
            if !key.starts_with(market_id) || pending.bot_id == decision.bot_id {
                continue;
            }
            if pending.action == decision.action {
                same_outcome_count += 1;
            }
        }

        let now = Self::now_ms();
        for (key, (outcome, ts)) in &self.recent_executions {
            if !key.starts_with(market_id) || now - ts > 5000 {
                continue;
            }
            if outcome == &decision.action {
                same_outcome_count += 1;
            }
        }

        if same_outcome_count >= self.config.max_bots_same_outcome as usize {
            return CoordinationResult {
                allowed: false,
                reason: format!("Max bots ({}) already positioned on {}",
                    self.config.max_bots_same_outcome, decision.action),
                adjusted_bet_size: None,
                warnings: None,
            };
        }

        CoordinationResult {
            allowed: true,
            reason: "Outcome capacity available".to_string(),
            adjusted_bet_size: None,
            warnings: None,
        }
    }

    fn cleanup_stale(&mut self, now: u64) {
        let stale_threshold = 5000; // 5 seconds
        self.pending_decisions.retain(|_, v| now - v.timestamp <= stale_threshold);
        self.recent_executions.retain(|_, (_, ts)| now - *ts <= stale_threshold);
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}
