use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::Row;

use crate::trading::market_condition::{detect_market_condition, MarketCondition, MarketRegime};
use super::AppState;

#[derive(Serialize)]
pub struct MarketConditionResponse {
    pub condition: MarketCondition,
    pub btc_price: f64,
    pub source: String,
}

#[derive(Serialize)]
pub struct BotRecommendation {
    pub bot_id: i64,
    pub bot_name: String,
    pub strategy: String,
    pub match_score: f64,
    pub reason: String,
}

#[derive(Serialize)]
pub struct MarketRecommendationResponse {
    pub condition: MarketCondition,
    pub recommendations: Vec<BotRecommendation>,
    pub summary: String,
}

pub async fn get_market_condition(
    State(_state): State<AppState>,
) -> Json<MarketConditionResponse> {
    let client = crate::http_client::build_http_client();
    let price_history = fetch_btc_price_history(&client).await;
    let condition = detect_market_condition(&price_history);
    let btc_price = price_history.last().map(|(p, _)| *p).unwrap_or(0.0);
    Json(MarketConditionResponse { condition, btc_price, source: "binance_klines".to_string() })
}

pub async fn get_market_recommendation(
    State(state): State<AppState>,
) -> Json<MarketRecommendationResponse> {
    let client = crate::http_client::build_http_client();
    let price_history = fetch_btc_price_history(&client).await;
    let condition = detect_market_condition(&price_history);
    let db = state.db();
    let bots = get_all_bots_for_recommendation(&db).await;
    let recommendations = recommend_bots(&condition, &bots);
    let summary = format!("Piaci allapot: {} | Ajanlott botok: {}", condition.regime, recommendations.len());
    Json(MarketRecommendationResponse { condition, recommendations, summary })
}

async fn fetch_btc_price_history(client: &reqwest::Client) -> Vec<(f64, f64)> {
    let mut history = Vec::new();
    match client
        .get("https://api.binance.com/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=20")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(klines) = resp.json::<Vec<Vec<serde_json::Value>>>().await {
                for kline in &klines {
                    if let (Some(close), Some(time)) = (
                        kline.get(4).and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()),
                        kline.get(0).and_then(|v| v.as_f64())
                    ) {
                        history.push((close, time / 1000.0));
                    }
                }
            }
        }
        _ => {
            if let Ok(resp) = client
                .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(price) = data.get("price").and_then(|p| p.as_str()).and_then(|s| s.parse::<f64>().ok()) {
                        let now = chrono::Utc::now().timestamp() as f64;
                        history.push((price, now));
                    }
                }
            }
        }
    }
    history
}

#[derive(Debug, Clone)]
struct BotInfo {
    id: i64,
    name: String,
    strategy_type: String,
    params: String,
    status: String,
    win_rate: f64,
    total_pnl: f64,
}

async fn get_all_bots_for_recommendation(db: &crate::db::Db) -> Vec<BotInfo> {
    let rows = sqlx::query(
        "SELECT bc.id, bc.name, bc.strategy_type, bc.params, bc.status, 
                COALESCE(bp.winning_trades * 100.0 / NULLIF(bp.total_trades, 0), 0) as win_rate,
                COALESCE(bp.total_pnl, 0) as total_pnl
         FROM bot_configs bc
         LEFT JOIN bot_portfolios bp ON bc.id = bp.bot_id
         WHERE bc.category = 'crypto' OR bc.category IS NULL OR bc.category = ''
         ORDER BY bc.id"
    )
    .fetch_all(db.as_ref())
    .await
    .unwrap_or_default();
    
    rows.iter().map(|r| BotInfo {
        id: r.get("id"),
        name: r.get("name"),
        strategy_type: r.get("strategy_type"),
        params: r.get("params"),
        status: r.get("status"),
        win_rate: r.get("win_rate"),
        total_pnl: r.get("total_pnl"),
    }).collect()
}

fn recommend_bots(condition: &MarketCondition, bots: &[BotInfo]) -> Vec<BotRecommendation> {
    let mut recommendations: Vec<BotRecommendation> = bots.iter().map(|bot| {
        let strategy = &bot.strategy_type;
        let match_score = calculate_match_score(strategy, condition);
        let reason = generate_match_reason(strategy, condition, bot);
        BotRecommendation { bot_id: bot.id, bot_name: bot.name.clone(), strategy: strategy.clone(), match_score, reason }
    }).collect();
    recommendations.sort_by(|a, b| b.match_score.partial_cmp(&a.match_score).unwrap_or(std::cmp::Ordering::Equal));
    recommendations.truncate(5);
    recommendations
}

fn calculate_match_score(strategy: &str, condition: &MarketCondition) -> f64 {
    let base_score = match condition.regime {
        MarketRegime::Trending => match strategy {
            "momentum" | "momentum_v2" | "strict_momentum" => 0.9,
            "trend" | "trend_pullback" => 0.85,
            "binance_velocity" | "binance_velocity_v2" => 0.7,
            "contrarian" => 0.2,
            "mean_reversion" | "mean_reversion_v2" => 0.3,
            "volatility" | "volatility_filtered" => 0.5,
            "sniper" | "sniper_value" | "sniper_arb" => 0.6,
            "edge_hunter" | "extreme_edge" => 0.4,
            _ => 0.5,
        },
        MarketRegime::Ranging => match strategy {
            "contrarian" => 0.85,
            "edge_hunter" | "extreme_edge" => 0.8,
            "mean_reversion" | "mean_reversion_v2" => 0.75,
            "price_reversion" | "price_reversion_v2" => 0.7,
            "patient_waiter" => 0.7,
            "sniper_value" | "ultra_low_entry" | "odds_swing" => 0.65,
            "momentum" | "momentum_v2" | "strict_momentum" => 0.3,
            "trend" | "trend_pullback" => 0.3,
            "volatility" | "volatility_filtered" => 0.4,
            _ => 0.5,
        },
        MarketRegime::Volatile => match strategy {
            "volatility" | "volatility_filtered" => 0.85,
            "sniper" | "sniper_value" | "sniper_arb" => 0.75,
            "binance_velocity" | "binance_velocity_v2" => 0.7,
            "extreme_edge" => 0.65,
            "momentum" | "momentum_v2" => 0.6,
            "contrarian" => 0.5,
            "mean_reversion" | "mean_reversion_v2" => 0.4,
            _ => 0.5,
        },
        MarketRegime::Unknown => 0.5,
    };
    let vol_adjustment = match condition.volatility {
        v if v > 0.7 => match strategy {
            "volatility" | "volatility_filtered" | "sniper" | "sniper_value" => 0.1,
            "contrarian" | "mean_reversion" => -0.1,
            _ => 0.0,
        },
        v if v < 0.3 => match strategy {
            "contrarian" | "edge_hunter" | "mean_reversion" | "patient_waiter" => 0.1,
            "volatility" | "volatility_filtered" => -0.1,
            _ => 0.0,
        },
        _ => 0.0,
    };
    let score: f64 = base_score + vol_adjustment;
    score.clamp(0.0, 1.0)
}

fn generate_match_reason(strategy: &str, condition: &MarketCondition, bot: &BotInfo) -> String {
    let regime_desc = match condition.regime {
        MarketRegime::Trending => "trendelo",
        MarketRegime::Ranging => "oldalazo",
        MarketRegime::Volatile => "volatilis",
        MarketRegime::Unknown => "ismeretlen",
    };
    let perf_desc = if bot.win_rate > 60.0 {
        format!(" (nyeresi arany: {:.0}%)", bot.win_rate)
    } else if bot.total_pnl > 0.0 {
        format!(" (P&L: +${:.2})", bot.total_pnl)
    } else if bot.total_pnl < 0.0 {
        format!(" (P&L: ${:.2})", bot.total_pnl)
    } else {
        String::new()
    };
    format!("{} {} piacra - {}{}", strategy_name(strategy), regime_desc, strategy_match_reason(strategy, &condition.regime), perf_desc)
}

fn strategy_name(strategy: &str) -> &str {
    match strategy {
        "momentum" => "Momentum",
        "momentum_v2" => "Momentum V2",
        "strict_momentum" => "Strict Momentum",
        "trend" => "Trend",
        "trend_pullback" => "Trend Pullback",
        "contrarian" => "Contrarian",
        "mean_reversion" => "Mean Reversion",
        "mean_reversion_v2" => "Mean Reversion V2",
        "volatility" => "Volatility",
        "volatility_filtered" => "Volatility Filtered",
        "sniper" => "Sniper",
        "sniper_value" => "Sniper Value",
        "sniper_arb" => "Sniper Arb",
        "edge_hunter" => "Edge Hunter",
        "extreme_edge" => "Extreme Edge",
        "binance_velocity" => "Binance Velocity",
        "binance_velocity_v2" => "Binance Velocity V2",
        "fair_value" => "Fair Value",
        "oracle_lag" => "Oracle Lag",
        "oracle_lag_v2" => "Oracle Lag V2",
        "low_volatility_edge" => "Low Vol Edge",
        "patient_waiter" => "Patient Waiter",
        "ultra_low_entry" => "Ultra Low Entry",
        "odds_swing" => "Odds Swing",
        "price_reversion" => "Price Reversion",
        "price_reversion_v2" => "Price Reversion V2",
        other => other,
    }
}

fn strategy_match_reason<'a>(strategy: &'a str, regime: &MarketRegime) -> &'a str {
    match (strategy, regime) {
        ("momentum" | "momentum_v2" | "strict_momentum", MarketRegime::Trending) => "eros trend -> momentum kovetes",
        ("trend" | "trend_pullback", MarketRegime::Trending) => "trend folytatasa varhato",
        ("contrarian", MarketRegime::Ranging) => "oldalazas -> visszafordulasok",
        ("edge_hunter" | "extreme_edge", MarketRegime::Ranging) => "kis mozgasok -> elvadaszat",
        ("mean_reversion" | "mean_reversion_v2", MarketRegime::Ranging) => "atlagoz visszateres",
        ("volatility" | "volatility_filtered", MarketRegime::Volatile) => "nagy hullamzas -> kitores",
        ("sniper" | "sniper_value", MarketRegime::Volatile) => "gyors belepes nagy mozgasnal",
        ("binance_velocity" | "binance_velocity_v2", MarketRegime::Volatile) => "sebesseg kihasznalasa",
        _ => "altalanos illeszkeredes",
    }
}
