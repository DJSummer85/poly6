//! Bot Loss Tracker - Per-bot consecutive loss and drawdown tracking
//! Ported from polymarket-demo/src/lib/bot-manager/strategy-executor.ts

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BotLossTracker {
    pub consecutive_losses: u32,
    pub total_losses: u32,
    pub total_wins: u32,
    pub last_loss_time: Option<i64>,
    pub drawdown: f64,
    pub peak_balance: f64,
    pub pending_settlements: u32,
}

pub struct BotLossTrackerManager {
    trackers: HashMap<i64, BotLossTracker>,
}

impl BotLossTrackerManager {
    pub fn new() -> Self {
        Self {
            trackers: HashMap::new(),
        }
    }

    fn get_or_create(&mut self, bot_id: i64, current_balance: f64) -> &BotLossTracker {
        self.trackers.entry(bot_id).or_insert_with(|| BotLossTracker {
            consecutive_losses: 0,
            total_losses: 0,
            total_wins: 0,
            last_loss_time: None,
            drawdown: 0.0,
            peak_balance: current_balance,
            pending_settlements: 0,
        });

        // Update peak and drawdown
        let tracker = self.trackers.get(&bot_id).unwrap();
        let peak = tracker.peak_balance.max(current_balance);
        let drawdown = if peak > 0.0 {
            ((peak - current_balance) / peak) * 100.0
        } else {
            0.0
        };
        let tracker = self.trackers.get_mut(&bot_id).unwrap();
        tracker.peak_balance = peak;
        tracker.drawdown = drawdown;

        self.trackers.get(&bot_id).unwrap()
    }

    /// Mark a trade as sent (increments pending settlements)
    pub fn mark_trade_sent(&mut self, bot_id: i64) {
        if let Some(tracker) = self.trackers.get_mut(&bot_id) {
            tracker.pending_settlements += 1;
            tracing::debug!("[LossTracker] bot {}: trade sent, pending={}", bot_id, tracker.pending_settlements);
        }
    }

    /// Update tracker after settlement
    pub fn update_settlement(&mut self, bot_id: i64, won: bool, pnl: f64, current_balance: f64) {
        let prev_consecutive = {
            let tracker = self.get_or_create(bot_id, current_balance);
            tracker.consecutive_losses
        };

        let tracker = self.trackers.get_mut(&bot_id).unwrap();

        if won {
            tracker.total_wins += 1;
            tracker.consecutive_losses = 0;
        } else {
            tracker.total_losses += 1;
            tracker.consecutive_losses += 1;
            tracker.last_loss_time = Some(chrono::Utc::now().timestamp());
        }

        tracker.pending_settlements = tracker.pending_settlements.saturating_sub(1);

        tracing::debug!(
            "[LossTracker] bot {} settlement: {} ({:+.2}) | consecutiveLosses: {} -> {}, pending: {}",
            bot_id,
            if won { "WIN" } else { "LOSS" },
            pnl,
            prev_consecutive,
            tracker.consecutive_losses,
            tracker.pending_settlements,
        );
    }

    /// Get risk multiplier based on bot performance
    /// Returns 0.0 if bot should stop trading, or a multiplier (0.25, 0.5, 0.75, 1.0)
    pub fn get_risk_multiplier(&mut self, bot_id: i64, current_balance: f64) -> f64 {
        let _tracker = self.get_or_create(bot_id, current_balance);

        let tracker = self.trackers.get(&bot_id).unwrap();

        // Logging
        if tracker.consecutive_losses > 0 || tracker.pending_settlements > 0 {
            tracing::debug!(
                "[LossTracker] bot {}: consecutive_losses={}, pending={}, drawdown={:.1}%",
                bot_id, tracker.consecutive_losses, tracker.pending_settlements, tracker.drawdown
            );
        }

        // 5+ consecutive losses -> very aggressive reduction
        if tracker.consecutive_losses >= 5 {
            return 0.25;
        }

        // 2 consecutive losses -> 25% of normal size
        if tracker.consecutive_losses == 2 {
            return 0.25;
        }

        // 1 consecutive loss -> 50% of normal size
        if tracker.consecutive_losses == 1 {
            return 0.5;
        }

        // Drawdown-based reduction
        if tracker.drawdown >= 30.0 {
            return 0.25;
        }

        if tracker.drawdown >= 20.0 {
            return 0.5;
        }

        1.0 // Normal sizing
    }

    /// Adjust confidence based on recent performance
    pub fn adjust_confidence(&mut self, bot_id: i64, base_confidence: f64, current_balance: f64) -> f64 {
        let _tracker = self.get_or_create(bot_id, current_balance);
        let tracker = self.trackers.get(&bot_id).unwrap();

        let mut multiplier = 1.0;

        match tracker.consecutive_losses {
            1 => multiplier = 0.7,
            2 => multiplier = 0.5,
            3..=99 => multiplier = 0.3,
            _ => {}
        }

        // Additional penalty if overall losing streak
        if tracker.total_wins >= 3 && tracker.total_losses > (tracker.total_wins as f64 * 1.5) as u32 {
            multiplier *= 0.8;
        }

        (base_confidence * multiplier).max(0.0)
    }

    /// Reset all trackers
    pub fn reset_all(&mut self) {
        self.trackers.clear();
    }

    /// Get tracker info for a bot
    pub fn get_tracker_info(&mut self, bot_id: i64, current_balance: f64) -> BotTrackerInfo {
        let _tracker = self.get_or_create(bot_id, current_balance);
        let tracker = self.trackers.get(&bot_id).unwrap();
        BotTrackerInfo {
            consecutive_losses: tracker.consecutive_losses,
            total_losses: tracker.total_losses,
            total_wins: tracker.total_wins,
            drawdown: tracker.drawdown,
            pending_settlements: tracker.pending_settlements,
            peak_balance: tracker.peak_balance,
        }
    }

    /// Clear a bot's tracker
    pub fn clear_bot(&mut self, bot_id: i64) {
        self.trackers.remove(&bot_id);
    }
}

impl Default for BotLossTrackerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct BotTrackerInfo {
    pub consecutive_losses: u32,
    pub total_losses: u32,
    pub total_wins: u32,
    pub drawdown: f64,
    pub pending_settlements: u32,
    pub peak_balance: f64,
}
