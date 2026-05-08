//! Oracle Lag Strategy - Exploits Chainlink oracle delay
//!
//! Uses the 4-12 second delay between Binance and Polymarket oracle

use super::base::{
    check_price_limits, check_time_remaining,
    Signal, Strategy, StrategyContext, StrategyDecision, StrategyParams,
};

pub struct OracleLagStrategy {
    params: StrategyParams,
}

impl OracleLagStrategy {
    pub fn new(params: StrategyParams) -> Self {
        Self { params }
    }
}

impl Default for OracleLagStrategy {
    fn default() -> Self {
        Self {
            params: StrategyParams {
                min_delta: 0.015, // Lower threshold - exploits small delays
                min_price: 0.30,
                max_price: 0.70,
                ..Default::default()
            },
        }
    }
}

impl Strategy for OracleLagStrategy {
    fn name(&self) -> &str {
        "Oracle Lag"
    }

    fn description(&self) -> &str {
        "Exploits the 4-12 second delay in Chainlink oracle updates"
    }

    fn evaluate(&self, ctx: &StrategyContext) -> StrategyDecision {
        if !check_time_remaining(ctx.time_remaining, &self.params) {
            return StrategyDecision::hold("Too close to market close");
        }

        let pm_price = match ctx.polymarket_price {
            Some(p) => p,
            None => return StrategyDecision::hold("No PM price"),
        };

        if !check_price_limits(pm_price, &self.params) {
            return StrategyDecision::hold("Price outside range");
        }

        // Oracle lag looks at BTC price change - the movement that hasn't been reflected yet
        let btc_change = match ctx.btc_price_change {
            Some(c) => c,
            None => return StrategyDecision::hold("No BTC data for oracle lag"),
        };

        let change_pct = btc_change * 100.0;

        // Small but significant moves that oracle hasn't caught yet
        if change_pct > self.params.min_delta {
            return StrategyDecision::trade(
                Signal::Yes,
                0.72,
                &format!("Oracle lag: BTC moved +{:.2}%, oracle lagging", change_pct),
            );
        }

        if change_pct < -self.params.min_delta {
            return StrategyDecision::trade(
                Signal::No,
                0.72,
                &format!("Oracle lag: BTC moved {:.2}%, oracle lagging", change_pct),
            );
        }

        StrategyDecision::hold("No oracle lag opportunity")
    }
}
