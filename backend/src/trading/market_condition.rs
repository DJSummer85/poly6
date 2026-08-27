//! Market Condition Detector
//!
//! Analyzes BTC price history to determine the current market regime:
//! - TRENDING: Clear directional movement (momentum strategies work best)
//! - RANGING: Sideways movement, low volatility (contrarian strategies work best)
//! - VOLATILE: High amplitude swings (volatility strategies work best)
//! - UNKNOWN: Insufficient data

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MarketRegime {
    Trending,
    Ranging,
    Volatile,
    Unknown,
}

impl std::fmt::Display for MarketRegime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketRegime::Trending => write!(f, "Trending"),
            MarketRegime::Ranging => write!(f, "Ranging"),
            MarketRegime::Volatile => write!(f, "Volatile"),
            MarketRegime::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCondition {
    pub regime: MarketRegime,
    pub confidence: f64,
    pub trend_strength: f64,
    pub volatility: f64,
    pub avg_price_change: f64,
    pub price_range_pct: f64,
    pub velocity: f64,
    pub acceleration: f64,
    pub recommended_strategies: Vec<StrategyRecommendation>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRecommendation {
    pub strategy: String,
    pub name: String,
    pub reason: String,
    pub suitability: f64,
}

pub fn detect_market_condition(price_history: &[(f64, f64)]) -> MarketCondition {
    if price_history.len() < 3 {
        return MarketCondition {
            regime: MarketRegime::Unknown,
            confidence: 0.0,
            trend_strength: 0.0,
            volatility: 0.0,
            avg_price_change: 0.0,
            price_range_pct: 0.0,
            velocity: 0.0,
            acceleration: 0.0,
            recommended_strategies: vec![],
            summary: "Nincs eleg adat a piaci allapot megallapitasahoz".to_string(),
        };
    }

    let current_price = price_history.last().unwrap().0;
    let start_price = price_history.first().unwrap().0;

    // Calculate individual returns
    let mut returns: Vec<f64> = Vec::new();
    for i in 1..price_history.len() {
        let prev = price_history[i - 1].0;
        let curr = price_history[i].0;
        if prev > 0.0 {
            returns.push((curr - prev) / prev);
        }
    }

    // Net change from start to end
    let net_change = (current_price - start_price) / start_price;

    // Volatility: standard deviation of returns
    let mean_return = if returns.is_empty() {
        0.0
    } else {
        returns.iter().sum::<f64>() / returns.len() as f64
    };
    let variance = if returns.is_empty() {
        0.0
    } else {
        returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / returns.len() as f64
    };
    let std_dev = variance.sqrt();
    let volatility = (std_dev * 100.0).min(5.0) / 5.0;

    // Price range
    let high = price_history.iter().map(|(p, _)| *p).fold(f64::NEG_INFINITY, f64::max);
    let low = price_history.iter().map(|(p, _)| *p).fold(f64::INFINITY, f64::min);
    let price_range_pct = if current_price > 0.0 {
        (high - low) / current_price
    } else {
        0.0
    };

    // Velocity: net change per second
    let total_duration = price_history.last().unwrap().1 - price_history.first().unwrap().1;
    let velocity = if total_duration > 0.0 {
        net_change / total_duration
    } else {
        0.0
    };

    // Acceleration: change in velocity (simplified)
    let mid_idx = price_history.len() / 2;
    let mid_price = price_history[mid_idx].0;
    let first_half_change = (mid_price - start_price) / start_price;
    let second_half_change = (current_price - mid_price) / mid_price;
    let first_half_time = price_history[mid_idx].1 - price_history.first().unwrap().1;
    let second_half_time = price_history.last().unwrap().1 - price_history[mid_idx].1;
    let v1 = if first_half_time > 0.0 {
        first_half_change / first_half_time
    } else {
        0.0
    };
    let v2 = if second_half_time > 0.0 {
        second_half_change / second_half_time
    } else {
        0.0
    };
    let acceleration = v2 - v1;

    // Directional consistency
    let total_returns = returns.len() as f64;
    let positive_returns = returns.iter().filter(|&&r| r > 0.0).count() as f64;
    let negative_returns = returns.iter().filter(|&&r| r < 0.0).count() as f64;
    let directional_consistency = if total_returns > 0.0 {
        positive_returns.max(negative_returns) / total_returns
    } else {
        0.5
    };

    let (regime, confidence) = determine_regime(
        net_change,
        std_dev,
        volatility,
        directional_consistency,
        price_range_pct,
    );
    let recommended_strategies = recommend_strategies(&regime, volatility, net_change);
    let summary = generate_summary(&regime, net_change, volatility, price_range_pct);

    MarketCondition {
        regime,
        confidence,
        trend_strength: net_change.clamp(-1.0, 1.0),
        volatility,
        avg_price_change: net_change * 100.0,
        price_range_pct: price_range_pct * 100.0,
        velocity,
        acceleration,
        recommended_strategies,
        summary,
    }
}

fn determine_regime(
    net_change: f64,
    std_dev: f64,
    volatility: f64,
    directional_consistency: f64,
    price_range_pct: f64,
) -> (MarketRegime, f64) {
    let abs_change = net_change.abs();

    // Strong trend
    if abs_change > 0.002 && directional_consistency > 0.65 {
        let conf = (0.5 + abs_change * 100.0 + directional_consistency * 0.3).min(1.0);
        return (MarketRegime::Trending, conf);
    }

    // High volatility
    if price_range_pct > 0.003 && std_dev > 0.001 {
        let conf = (0.4 + volatility * 0.5 + price_range_pct * 50.0).min(1.0);
        return (MarketRegime::Volatile, conf);
    }

    // Ranging
    if abs_change < 0.001 && std_dev < 0.0008 {
        let conf = (0.5 + (1.0 - volatility) * 0.3 + (1.0 - directional_consistency) * 0.2).min(1.0);
        return (MarketRegime::Ranging, conf);
    }

    let conf = (0.3 + (1.0 - abs_change * 100.0).max(0.0) * 0.3).min(1.0);
    (MarketRegime::Ranging, conf)
}

fn recommend_strategies(
    regime: &MarketRegime,
    _volatility: f64,
    net_change: f64,
) -> Vec<StrategyRecommendation> {
    match regime {
        MarketRegime::Trending => {
            let mut recs = vec![
                StrategyRecommendation {
                    strategy: "momentum".to_string(),
                    name: "Momentum".to_string(),
                    reason: "Eros trend -> momentum kovetes a legjobb".to_string(),
                    suitability: 0.9,
                },
                StrategyRecommendation {
                    strategy: "trend".to_string(),
                    name: "Trend".to_string(),
                    reason: "Trend koveto — irany megtartasa varhato".to_string(),
                    suitability: 0.85,
                },
                StrategyRecommendation {
                    strategy: "momentum_v2".to_string(),
                    name: "Momentum V2".to_string(),
                    reason: "Fejlesztett momentum kockazat-kalibralassal".to_string(),
                    suitability: 0.8,
                },
            ];
            if net_change > 0.001 {
                recs.push(StrategyRecommendation {
                    strategy: "strict_momentum".to_string(),
                    name: "Strict Momentum".to_string(),
                    reason: "Eros felfele mozgas — szigoru szures".to_string(),
                    suitability: 0.75,
                });
            }
            recs
        }
        MarketRegime::Ranging => vec![
            StrategyRecommendation {
                strategy: "contrarian".to_string(),
                name: "Contrarian".to_string(),
                reason: "Oldalazo piac -> visszafordulasokra jatszunk".to_string(),
                suitability: 0.85,
            },
            StrategyRecommendation {
                strategy: "edge_hunter".to_string(),
                name: "Edge Hunter".to_string(),
                reason: "Kis mozgasok -> valoszinusegi el".to_string(),
                suitability: 0.8,
            },
            StrategyRecommendation {
                strategy: "mean_reversion".to_string(),
                name: "Mean Reversion".to_string(),
                reason: "Atlagoz visszateres — oldalazo piacon hatekony".to_string(),
                suitability: 0.75,
            },
            StrategyRecommendation {
                strategy: "patient_waiter".to_string(),
                name: "Patient Waiter".to_string(),
                reason: "Varakozas tokeletes 50c setupra".to_string(),
                suitability: 0.7,
            },
        ],
        MarketRegime::Volatile => vec![
            StrategyRecommendation {
                strategy: "volatility".to_string(),
                name: "Volatility".to_string(),
                reason: "Magas volatilitas -> kitoresi strategiak".to_string(),
                suitability: 0.85,
            },
            StrategyRecommendation {
                strategy: "volatility_filtered".to_string(),
                name: "Volatility Filtered".to_string(),
                reason: "Szurt volatilitas — eros jelekre kereskedik".to_string(),
                suitability: 0.8,
            },
            StrategyRecommendation {
                strategy: "sniper".to_string(),
                name: "Sniper".to_string(),
                reason: "Alacsony belepes nagy mozgasoknal".to_string(),
                suitability: 0.75,
            },
        ],
        MarketRegime::Unknown => vec![
            StrategyRecommendation {
                strategy: "momentum".to_string(),
                name: "Momentum".to_string(),
                reason: "Alapertelmezett — altalaban megbizhato".to_string(),
                suitability: 0.5,
            },
            StrategyRecommendation {
                strategy: "contrarian".to_string(),
                name: "Contrarian".to_string(),
                reason: "Alternativa — visszafordulasokra".to_string(),
                suitability: 0.5,
            },
        ],
    }
}

fn generate_summary(
    regime: &MarketRegime,
    net_change: f64,
    volatility: f64,
    price_range_pct: f64,
) -> String {
    let direction = if net_change > 0.0005 {
        "felfele"
    } else if net_change < -0.0005 {
        "lefele"
    } else {
        "oldalazik"
    };
    let vol_desc = if volatility > 0.6 {
        "magas"
    } else if volatility > 0.3 {
        "kozepes"
    } else {
        "alacsony"
    };

    match regime {
        MarketRegime::Trending => format!(
            "TREND — A piac {} mozog ({:.2}%). {} volatilitas. Momentum strategiak a legjobbak.",
            direction,
            net_change * 100.0,
            vol_desc
        ),
        MarketRegime::Ranging => format!(
            "RANGING — A piac {} ({:.2}% elmozdulas). {} volatilitas. Contrarian/edge hunter a legjobb.",
            direction,
            net_change * 100.0,
            vol_desc
        ),
        MarketRegime::Volatile => format!(
            "VOLATILE — Nagy hullamzas ({:.2}% tartomany). {} volatilitas. Volatility strategiak a legjobb.",
            price_range_pct * 100.0,
            vol_desc
        ),
        MarketRegime::Unknown => {
            "Ismeretlen — nincs eleg adat. Alapertelmezett strategiakkal kereskedik.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trending_market() {
        let history: Vec<(f64, f64)> = (0..20)
            .map(|i| (78000.0 + i as f64 * 10.0, i as f64 * 3.0))
            .collect();
        let condition = detect_market_condition(&history);
        assert_eq!(condition.regime, MarketRegime::Trending);
        assert!(condition.confidence > 0.5);
    }

    #[test]
    fn test_ranging_market() {
        let history: Vec<(f64, f64)> = (0..20)
            .map(|i| (78000.0 + (i as f64 * 0.5).sin() * 5.0, i as f64 * 3.0))
            .collect();
        let condition = detect_market_condition(&history);
        assert_eq!(condition.regime, MarketRegime::Ranging);
    }

    #[test]
    fn test_insufficient_data() {
        let history: Vec<(f64, f64)> = vec![(78000.0, 0.0), (78001.0, 3.0)];
        let condition = detect_market_condition(&history);
        assert_eq!(condition.regime, MarketRegime::Unknown);
    }
}
