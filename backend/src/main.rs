#![allow(dead_code)]

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use sqlx::Row;

mod api;
mod crypto;
mod db;
mod middleware;
mod services;
mod trading;

use api::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "polymarket_v2_backend=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Polymarket V2 Backend");

    let db = db::init_db().await?;
    let app_state = AppState::new(db.clone());

    // === AUTO-LOAD CREDENTIALS ===
    {
        let pool = db.as_ref();
        let users = sqlx::query("SELECT id FROM users").fetch_all(pool).await.unwrap_or_default();
        for user_row in users {
            let user_id: i64 = user_row.get("id");
            if let Ok(keys) = db::queries::get_api_keys(&db, user_id).await {
                let pk = keys.iter().find(|k| k.key_name == "polymarket_private_key").map(|k| k.key_value.clone());
                if let Some(private_key) = pk {
                    let mut cache = app_state.credential_cache.write().await;
                    cache.insert(user_id, api::CachedCredentials {
                        api_key: keys.iter().find(|k| k.key_name == "polymarket_api_key").map(|k| k.key_value.clone()).unwrap_or_default(),
                        api_secret: keys.iter().find(|k| k.key_name == "polymarket_api_secret").map(|k| k.key_value.clone()).unwrap_or_default(),
                        api_passphrase: keys.iter().find(|k| k.key_name == "polymarket_passphrase").map(|k| k.key_value.clone()).unwrap_or_default(),
                        private_key, funder: None, signature_type: 0, wallet_address: String::new(),
                    });
                }
            }
        }
    }

    // === AUTO-LOAD RUNNING BOTS ON STARTUP ===
    {
        let pool = db.as_ref();
        let active_bots = sqlx::query("SELECT id, user_id FROM bot_configs WHERE status = 'running'")
            .fetch_all(pool).await.unwrap_or_default();

        for bot_row in active_bots {
            let bot_id: i64 = bot_row.get("id");
            let user_id: i64 = bot_row.get("user_id");
            
            if let Ok(Some(bot_rec)) = db::queries::get_bot_by_id(&db, bot_id, user_id).await {
                if let Ok(Some(portfolio)) = db::queries::get_portfolio(&db, bot_id, user_id).await {
                    let _ = app_state.orchestrator.resume_bot(&bot_rec, portfolio.balance).await;
                    let orch = app_state.orchestrator.clone();
                    let cache = Some(app_state.credential_cache.clone());
                    tokio::spawn(async move {
                        trading::orchestrator::start_orchestrator_loop(orch, bot_id, user_id, 5, cache).await;
                    });
                }
            }
        }
    }

    // Event broadcaster
    let event_receiver = app_state.event_receiver.clone();
    let broadcaster = app_state.bot_event_broadcaster.clone();
    tokio::spawn(async move {
        let mut rx = event_receiver.write().await;
        while let Some(event) = rx.recv().await { let _ = broadcaster.send(event); }
    });

// Restore any running bots from previous session
    // This restarts bots that were running before the server was stopped
    let restore_orchestrator = app_state.orchestrator.clone();
    tokio::spawn(async move {
        trading::orchestrator::restore_running_bots(restore_orchestrator).await;
    });
    tracing::info!("Bot restore from database started");

    // Auto-save loop
    let orch_save = app_state.orchestrator.clone();
    tokio::spawn(async move { trading::orchestrator::start_auto_save_loop(orch_save).await; });

    // CORS layer

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", api::routes(app_state.clone()))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::info!("Listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}