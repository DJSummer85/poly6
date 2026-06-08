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

    // === AUTO-LOAD CREDENTIALS (csak a cache-t tölti fel, botokat NEM indít) ===
    {
        let pool = db.as_ref();
        let users = sqlx::query("SELECT id FROM users").fetch_all(pool).await.unwrap_or_default();
        let mut loaded_count = 0usize;
        let mut incomplete_count = 0usize;
        for user_row in users {
            let user_id: i64 = user_row.get("id");
            if let Ok(keys) = db::queries::get_api_keys(&db, user_id).await {
                let pk = keys.iter().find(|k| k.key_name == "polymarket_private_key").map(|k| k.key_value.clone());
                let api_key_val = keys.iter().find(|k| k.key_name == "polymarket_api_key").map(|k| k.key_value.clone()).unwrap_or_default();
                let api_secret_val = keys.iter().find(|k| k.key_name == "polymarket_api_secret").map(|k| k.key_value.clone()).unwrap_or_default();
                let api_passphrase_val = keys.iter().find(|k| k.key_name == "polymarket_passphrase").map(|k| k.key_value.clone()).unwrap_or_default();

                if let Some(private_key) = pk {
                    // FIX: Ha bármely HMAC kulcs hiányzik, logoljuk részletesen és ugorjuk át
                    if api_key_val.len() < 5 || api_secret_val.len() < 5 || api_passphrase_val.len() < 5 {
                        tracing::warn!(
                            "User {} has a private key but incomplete HMAC credentials (api_key={}, secret={}, passphrase={}). \
                             Live trading will be blocked until all credentials are re-saved in Settings.",
                            user_id,
                            if api_key_val.len() >= 5 { "OK" } else { "MISSING" },
                            if api_secret_val.len() >= 5 { "OK" } else { "MISSING" },
                            if api_passphrase_val.len() >= 5 { "OK" } else { "MISSING" },
                        );
                        incomplete_count += 1;
                        continue;
                    }

                    // Read stored settings from api_keys table
                    let funder = keys.iter().find(|k| k.key_name == "polymarket_funder").map(|k| k.key_value.clone());
                    let signature_type_str = keys.iter().find(|k| k.key_name == "polymarket_signature_type").map(|k| k.key_value.clone());
                    let signature_type: u8 = signature_type_str.and_then(|s| s.parse().ok()).unwrap_or(0);
                    let wallet_address = match trading::PolymarketClient::new(&private_key) {
                        Ok(client) => client.address(),
                        Err(e) => {
                            tracing::error!("User {} has invalid private key stored: {}", user_id, e);
                            incomplete_count += 1;
                            continue;
                        }
                    };

                    // Always load stored deposit wallet address if available.
                    // The orchestrator uses it for POLY_1271 (signature_type=3) order signing.
                    // The HMAC auth uses the funder address (which matches the funder's API key).
                    let deposit_wallet_address = keys.iter()
                        .find(|k| k.key_name == "polymarket_deposit_wallet_address")
                        .map(|k| k.key_value.clone());

                    // If secret is empty (V2 key without secret/passphrase), try to derive
                    // Use the FULL derived credentials from the CLOB (key, secret, passphrase)
                    // because the user's website-created key might be registered to a different
                    // address than the wallet. The derived key will be registered to the wallet.
                    let (final_key, final_secret, final_passphrase) = if api_secret_val.is_empty() && !api_key_val.is_empty() {
                        match trading::polymarket::derive_api_key_for_private_key(&private_key).await {
                            Ok(creds) => {
                                tracing::info!("Derived full CLOB credentials: key={}", creds.key);
                                let pool = db.as_ref();
                                sqlx::query("INSERT OR REPLACE INTO api_keys (user_id, key_name, key_value) VALUES (?, 'polymarket_api_key', ?)")
                                    .bind(user_id).bind(&creds.key).execute(pool).await.ok();
                                sqlx::query("INSERT OR REPLACE INTO api_keys (user_id, key_name, key_value) VALUES (?, 'polymarket_api_secret', ?)")
                                    .bind(user_id).bind(&creds.secret).execute(pool).await.ok();
                                sqlx::query("INSERT OR REPLACE INTO api_keys (user_id, key_name, key_value) VALUES (?, 'polymarket_passphrase', ?)")
                                    .bind(user_id).bind(&creds.passphrase).execute(pool).await.ok();
                                (creds.key, creds.secret, creds.passphrase)
                            }
                            Err(e) => {
                                tracing::warn!("Failed to derive API credentials for user {}: {}", user_id, e);
                                (api_key_val, api_secret_val, api_passphrase_val)
                            }
                        }
                    } else {
                        (api_key_val, api_secret_val, api_passphrase_val)
                    };

                    let mut cache = app_state.credential_cache.write().await;
                    cache.insert(user_id, api::CachedCredentials {
                        api_key: final_key,
                        api_secret: final_secret,
                        api_passphrase: final_passphrase,
                        private_key,
                        funder,
                        signature_type,
                        wallet_address: wallet_address.clone(),
                        deposit_wallet_address,
                    });
                    tracing::info!("Loaded credentials for user {} (wallet: {}, sig_type: {})", user_id, wallet_address, signature_type);
                    loaded_count += 1;
                }
            }
        }
        tracing::info!("Credential cache loaded: {} user(s) ready, {} incomplete (check Settings)", loaded_count, incomplete_count);
    }

    // === MINDEN BOT STÁTUSZÁT "stopped"-ra ÁLLÍTJUK INDULÁSKOR ===
    // A botokat kizárólag a felhasználó indíthatja el a dashboardon keresztül.
    // Ez megakadályozza, hogy az előző session "running" státuszai automatikusan újraindítsák a botokat.
    {
        let pool = db.as_ref();
        let result = sqlx::query("UPDATE bot_configs SET status = 'stopped' WHERE status = 'running'")
            .execute(pool)
            .await
            .unwrap_or_default();
        tracing::info!("Reset {} bot(s) to 'stopped' status on startup", result.rows_affected());
    }

    // Event broadcaster (SSE-hez kell, botok nélkül is fut)
    let event_receiver = app_state.event_receiver.clone();
    let broadcaster = app_state.bot_event_broadcaster.clone();
    tokio::spawn(async move {
        let mut rx = event_receiver.write().await;
        while let Some(event) = rx.recv().await { let _ = broadcaster.send(event); }
    });

    // Auto-save loop (üres ha nincs futó bot, de kell a struktúra miatt)
    let orch_save = app_state.orchestrator.clone();
    tokio::spawn(async move { trading::orchestrator::start_auto_save_loop(orch_save).await; });

    // CORS layer
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", api::routes(app_state.clone()))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::info!("Listening on {} — bots will only start when manually triggered", addr);

    // Use socket2 with SO_REUSEADDR to handle Windows TIME_WAIT on port
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    socket.set_reuse_address(true)?;
    socket.bind(&socket2::SockAddr::from(addr))?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    let std_listener: std::net::TcpListener = socket.into();
    let listener = tokio::net::TcpListener::from_std(std_listener)?;

    axum::serve(listener, app).await?;
    Ok(())
}
