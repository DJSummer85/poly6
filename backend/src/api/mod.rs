use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};

use crate::db::Db;
use crate::middleware::auth as auth_middleware;
use crate::services::CredentialService;
use crate::trading::BinanceClient;
use crate::trading::market_data::MarketDataService;
use crate::trading::orchestrator::{BotOrchestrator, BotEvent};

pub mod auth;
pub mod binance;
pub mod bots;
pub mod funding;
pub mod live_readiness;
pub mod market;
pub mod monitoring;
pub mod orders;
pub mod positions;
pub mod settings;
pub mod sse;
pub mod strategy_tests;
pub mod user;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub binance_client: Arc<RwLock<Option<BinanceClient>>>,
    pub orchestrator: Arc<BotOrchestrator>,
    pub event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<BotEvent>>>,
    /// Broadcast channel for SSE: multiple SSE connections can subscribe to bot events
    pub bot_event_broadcaster: Arc<broadcast::Sender<BotEvent>>,
    /// In-memory credential cache for live order execution (keyed by user_id)
    pub credential_cache: Arc<RwLock<HashMap<i64, CachedCredentials>>>,
    /// Centralized credential service for secure decrypt and cache
    pub credential_service: Arc<CredentialService>,
    /// Market data service for strategy evaluation
    pub market_service: MarketDataService,
}

/// Cached trading credentials (decrypted, kept in memory only)
#[derive(Clone)]
pub struct CachedCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub private_key: String,
    pub funder: Option<String>,
    pub signature_type: u8,
    pub wallet_address: String,
}

impl AppState {
    pub fn new(db: Db) -> Self {
        // Create event channel for orchestrator broadcasts (mpsc for single consumer)
        let (event_sender, event_receiver) = mpsc::unbounded_channel::<BotEvent>();

        // Create broadcast channel for SSE subscribers (many-to-many)
        let (broadcaster, _) = broadcast::channel(100);

        Self {
            db: db.clone(),
            binance_client: Arc::new(RwLock::new(None)),
            orchestrator: Arc::new(BotOrchestrator::new(db.clone(), event_sender)),
            event_receiver: Arc::new(RwLock::new(event_receiver)),
            bot_event_broadcaster: Arc::new(broadcaster),
            credential_cache: Arc::new(RwLock::new(HashMap::new())),
            credential_service: Arc::new(CredentialService::new()),
            market_service: crate::trading::market_data::MarketDataService::new(),
        }
    }

    pub fn db(&self) -> Db {
        self.db.clone()
    }
}

pub fn routes(app_state: AppState) -> Router<AppState> {
    // Public routes - no auth required
    let public_routes = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/market/btc-price", get(market::get_btc_price))
        .route("/market/price", get(market::get_market_price))
        .route("/market/list", get(market::list_markets))
        .route("/market/active", get(market::get_active_markets))
        .route("/events", get(sse::bot_events_stream))
        // Strategy Lab - public (list strategies only)
        .route("/strategies", get(strategy_tests::list_strategies))
        // Live trading - public (readiness check)
        .route("/live-readiness", get(live_readiness::get_live_readiness));

    // Protected routes - require JWT auth
    let protected_routes = Router::new()
        .route("/auth/me", get(auth::me))
        .route("/user/balance", get(user::get_user_balance))
        .route("/bots", post(bots::create_bot))
        .route("/bots", get(bots::list_bots))
        .route("/bots/:id", get(bots::get_bot))
        .route("/bots/:id", put(bots::update_bot))
        .route("/bots/:id", delete(bots::delete_bot))
        .route("/bots/:id/start", post(bots::start_bot))
        .route("/bots/:id/stop", post(bots::stop_bot))
        .route("/bots/:id/session", get(bots::get_session))
        .route("/bots/:id/portfolio", get(bots::get_portfolio))
        .route("/bots/:id/history", get(bots::get_history))
        .route("/bots/:id/trades", get(bots::get_trades))
        .route("/bots/:id/reset", post(bots::reset_bot))
        .route("/bots/:id/reset-demo", post(bots::reset_demo_balance))
        .route("/bots/stop-all", post(bots::stop_all_bots))
        .route("/bots/run-all", post(bots::run_all_bots))
        .route("/bots/set-mode", post(bots::set_all_bots_mode))
        .route("/portfolio", get(bots::get_aggregate_portfolio))
        .route("/bots/:id/status", get(monitoring::get_bot_status))
        .route("/orders", get(orders::list_orders))
        .route("/orders", post(orders::place_order))
        .route("/orders/quick", post(orders::quick_trade))
        .route("/orders/cancel", post(orders::cancel_order))
        .route("/positions", get(positions::list_positions))
        .route("/positions/live", get(orders::get_live_positions))
        .route("/settings", get(settings::get_settings))
        .route("/settings", put(settings::update_settings))
        .route("/settings/validate", post(settings::validate_key))
        .route("/settings/derive", post(settings::derive_key))
        .route("/settings/validate-existing", post(settings::validate_existing))
        .route("/settings/validate-with-balance", post(settings::validate_with_balance))
        .route("/settings/store", post(settings::store_key))
        .route("/settings/store-all", post(settings::store_credentials))
        .route("/settings/keys", get(settings::list_api_keys))
        .route("/settings/keys/:provider", delete(settings::delete_provider_keys))
        .route("/settings/keys/store", post(settings::store_api_keys))
        .route("/system/status", get(monitoring::get_system_status))
        .route("/system/logs", get(monitoring::get_logs))
        .route("/system/log", post(monitoring::log_activity))
        // Risk management
        .route("/risk/bots/:id", get(monitoring::get_bot_risk_status))
        .route("/risk/bots/:id/pause", post(monitoring::pause_bot_risk))
        .route("/risk/bots/:id/resume", post(monitoring::resume_bot_risk))
        .route("/risk/warnings", get(monitoring::get_risk_warnings))
        .route("/binance/start", post(binance::start_binance))
        .route("/binance/stop", post(binance::stop_binance))
        .route("/binance/price", get(binance::get_price))
        // Strategy Lab routes (protected)
        .route("/strategy-tests", post(strategy_tests::create_strategy_test))
        .route("/strategy-tests/:id", get(strategy_tests::get_strategy_test))
        .route("/strategy-tests/:id/events", get(strategy_tests::get_strategy_test_events))
        .route("/strategy-tests/:id/performance", get(strategy_tests::get_strategy_test_performance))
        // Live trading routes (protected)
        .route("/validate-credentials", axum::routing::post(live_readiness::validate_credentials))
        // Funding routes
        .route("/funding/info", get(funding::funding_info))
        .route("/funding/wallet-info", get(funding::wallet_info))
        .route("/funding/wrap", post(funding::wrap_pusd))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware::auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
}
