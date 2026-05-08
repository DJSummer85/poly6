use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub bot_id: i64,
    pub user_id: i64,
    pub market_id: String,
    pub side: PositionSide,
    pub size: f64,          // Number of shares
    pub avg_price: f64,     // Average entry price
    pub current_price: f64, // Current market price
    pub pnl: f64,           // Profit/Loss
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PositionSide {
    Buy,  // Long (YES)
    Sell, // Short (NO)
}

impl PositionSide {
    pub fn as_str(&self) -> &str {
        match self {
            PositionSide::Buy => "buy",
            PositionSide::Sell => "sell",
        }
    }
}

impl std::str::FromStr for PositionSide {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "buy" => Ok(PositionSide::Buy),
            "sell" => Ok(PositionSide::Sell),
            _ => Err(()),
        }
    }
}

pub struct PositionManager {
    positions: Vec<Position>,
}

impl PositionManager {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
        }
    }

    /// Calculate P&L for a position (static function)
    pub fn calculate_pnl_static(position: &Position) -> f64 {
        match position.side {
            PositionSide::Buy => {
                // Long position: profit when price goes up
                (position.current_price - position.avg_price) * position.size
            }
            PositionSide::Sell => {
                // Short position: profit when price goes down
                (position.avg_price - position.current_price) * position.size
            }
        }
    }

    /// Update current price for all positions in a market
    pub fn update_market_prices(&mut self, market_id: &str, current_price: f64) {
        for position in &mut self.positions {
            if position.market_id == market_id {
                position.current_price = current_price;
                position.pnl = Self::calculate_pnl_static(position);
            }
        }
    }

    /// Get total P&L across all positions
    pub fn total_pnl(&self) -> f64 {
        self.positions.iter().map(|p| p.pnl).sum()
    }

    /// Get position for a specific market
    pub fn get_market_position(&self, market_id: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.market_id == market_id)
    }

    /// Add a new position (after order fill)
    pub fn add_position(&mut self, position: Position) {
        self.positions.push(position);
    }

    /// Close a position (after order fill in opposite direction)
    pub fn close_position(&mut self, market_id: &str) -> Option<Position> {
        if let Some(pos_idx) = self.positions.iter().position(|p| p.market_id == market_id) {
            Some(self.positions.remove(pos_idx))
        } else {
            None
        }
    }

    /// Get all positions
    pub fn get_all(&self) -> &Vec<Position> {
        &self.positions
    }
}

impl Default for PositionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate position size based on Kelly criterion
pub fn kelly_size(bankroll: f64, win_prob: f64, odds: f64, fraction: f64) -> f64 {
    let expected_return = win_prob * odds - (1.0 - win_prob);
    let kelly = (bankroll * expected_return * fraction) / odds;
    kelly.max(0.0).min(bankroll * 0.25) // Cap at 25% of bankroll
}
