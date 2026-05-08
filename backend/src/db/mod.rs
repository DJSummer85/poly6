use std::sync::Arc;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

pub type Db = Arc<SqlitePool>;

pub async fn init_db() -> Result<Db, sqlx::Error> {
    let db_path = std::env::var("DATABASE_PATH")
        .unwrap_or_else(|_| "data/polymarket_v2.db".to_string());

    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    if !std::path::Path::new(&db_path).exists() {
        std::fs::File::create(&db_path).ok();
    }

    let db_url = if std::path::Path::new(&db_path).is_absolute() {
        format!("sqlite://{}", db_path)
    } else {
        format!("sqlite:{}", db_path)
    };

    println!("Connecting to database: {}", db_url);

    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent).ok();
        println!("Database path: {} (URL: {})", db_path, db_url);
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    run_migrations(&pool).await?;

    let db = Arc::new(pool);

    Ok(db)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bot_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            market_id TEXT NOT NULL,
            strategy_type TEXT NOT NULL DEFAULT 'btc_5min',
            params TEXT NOT NULL DEFAULT '{}',
            status TEXT NOT NULL DEFAULT 'stopped',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Prevent duplicate bot names per user
    // First, delete duplicates keeping only the most recently created bot for each (user_id, name)
    sqlx::query(
        r#"
        DELETE FROM bot_configs
        WHERE id NOT IN (
            SELECT MIN(id) FROM bot_configs
            GROUP BY user_id, name
        )
        "#,
    )
    .execute(pool)
    .await.ok(); // Best effort — if no duplicates, OK

    // Now create unique index
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_bot_configs_user_name
        ON bot_configs(user_id, name)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bot_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            market_id TEXT NOT NULL,
            side TEXT NOT NULL,
            price REAL NOT NULL,
            size REAL NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            order_id TEXT,
            filled_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS positions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bot_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            market_id TEXT NOT NULL,
            side TEXT NOT NULL,
            size REAL NOT NULL DEFAULT 0,
            avg_price REAL NOT NULL DEFAULT 0,
            current_price REAL NOT NULL DEFAULT 0,
            pnl REAL NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL UNIQUE,
            polymarket_api_key TEXT,
            encrypted_blob TEXT,
            funder_address TEXT,
            wallet_address TEXT,
            signature_type INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // API keys storage - generic key-value store for encrypted API credentials
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS api_keys (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            key_name TEXT NOT NULL,
            key_value TEXT NOT NULL,
            is_valid INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_validated TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES users(id),
            UNIQUE(user_id, key_name)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activity_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            bot_id INTEGER,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            metadata TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES users(id),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Bot sessions - tracking run periods with performance metrics
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bot_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bot_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT,
            start_balance REAL NOT NULL DEFAULT 100,
            end_balance REAL,
            total_trades INTEGER DEFAULT 0,
            winning_trades INTEGER DEFAULT 0,
            losing_trades INTEGER DEFAULT 0,
            total_pnl REAL DEFAULT 0,
            status TEXT DEFAULT 'running',
            max_drawdown REAL DEFAULT 0,
            strategy_config TEXT,
            mode TEXT NOT NULL DEFAULT 'demo',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Migration: add mode column if it doesn't exist (for existing databases)
    sqlx::query("ALTER TABLE bot_sessions ADD COLUMN mode TEXT NOT NULL DEFAULT 'demo'")
        .execute(pool)
        .await
        .ok(); // Ignore error if column already exists

    // bot_runs - complete run tracking with mode and balance
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bot_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bot_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            mode TEXT NOT NULL DEFAULT 'demo',
            status TEXT NOT NULL DEFAULT 'running',
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            ended_at TEXT,
            initial_balance REAL NOT NULL DEFAULT 100,
            final_balance REAL,
            realized_pnl REAL DEFAULT 0,
            unrealized_pnl REAL DEFAULT 0,
            total_trades INTEGER DEFAULT 0,
            winning_trades INTEGER DEFAULT 0,
            losing_trades INTEGER DEFAULT 0,
            max_drawdown REAL DEFAULT 0,
            strategy_config TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // trade_intents - strategy decisions (even if hold/rejected)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS trade_intents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER,
            bot_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            market_id TEXT NOT NULL,
            strategy_type TEXT NOT NULL,
            side TEXT NOT NULL,
            confidence REAL NOT NULL,
            reason TEXT,
            snapshot_json TEXT,
            risk_result TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_id) REFERENCES bot_runs(id),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // executions - paper or live order executions
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS executions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            intent_id INTEGER,
            run_id INTEGER,
            bot_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            mode TEXT NOT NULL DEFAULT 'demo',
            adapter TEXT NOT NULL DEFAULT 'paper',
            status TEXT NOT NULL DEFAULT 'pending',
            market_id TEXT NOT NULL,
            token_id TEXT,
            side TEXT NOT NULL,
            requested_size REAL NOT NULL,
            filled_size REAL DEFAULT 0,
            requested_price REAL,
            avg_fill_price REAL,
            external_order_id TEXT,
            error_code TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (intent_id) REFERENCES trade_intents(id),
            FOREIGN KEY (run_id) REFERENCES bot_runs(id),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Trade decisions - logging why bot made each decision
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS trade_decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bot_id INTEGER NOT NULL,
            session_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            market_slug TEXT NOT NULL,
            condition_id TEXT NOT NULL,
            outcome TEXT NOT NULL,
            signal_type TEXT NOT NULL,
            signal_confidence REAL NOT NULL,
            btc_price REAL,
            btc_change REAL,
            market_yes_price REAL,
            market_no_price REAL,
            time_remaining INTEGER,
            decision_reason TEXT,
            executed INTEGER DEFAULT 0,
            order_id TEXT,
            pnl REAL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (session_id) REFERENCES bot_sessions(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Bot portfolios - current state per bot
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bot_portfolios (
            bot_id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            balance REAL DEFAULT 100,
            initial_balance REAL DEFAULT 100,
            open_positions INTEGER DEFAULT 0,
            total_trades INTEGER DEFAULT 0,
            winning_trades INTEGER DEFAULT 0,
            losing_trades INTEGER DEFAULT 0,
            total_pnl REAL DEFAULT 0,
            peak_balance REAL DEFAULT 100,
            last_trade_time TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (bot_id) REFERENCES bot_configs(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create indexes for performance
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_bot ON bot_sessions(bot_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_status ON bot_sessions(status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_decisions_session ON trade_decisions(session_id)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_decisions_bot ON trade_decisions(bot_id)")
        .execute(pool)
        .await?;

    // Add new columns to bot_configs (for existing tables)
    // SQLite ignores errors if column already exists
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN bet_size REAL DEFAULT 1.0")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN use_kelly INTEGER DEFAULT 1")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN kelly_fraction REAL DEFAULT 0.25")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN max_bet REAL DEFAULT 0.25")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN interval INTEGER DEFAULT 60000")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN stop_loss REAL DEFAULT 0.1")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN take_profit REAL DEFAULT 0.2")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN total_trades INTEGER DEFAULT 0")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN winning_trades INTEGER DEFAULT 0")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN losing_trades INTEGER DEFAULT 0")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN win_rate REAL DEFAULT 0.0")
        .execute(pool)
        .await.ok();
    sqlx::query("ALTER TABLE bot_configs ADD COLUMN trading_mode TEXT DEFAULT 'paper'")
        .execute(pool)
        .await.ok();

    // Check if any bots exist — only seed on truly fresh install
    let bot_count: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM bot_configs")
        .fetch_one(pool)
        .await;

    if bot_count.map_or(true, |c| c == 0) {
        // Only seed if no users exist at all — this is a brand-new database
        let user_count: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await;

        if user_count.map_or(true, |c| c == 0) {
            // No users yet — bots will be seeded on first registration
            tracing::info!("Fresh install detected — default bots will be seeded on first registration");
        }
    }

    tracing::info!("Database migrations completed");
    Ok(())
}

/// Seed 15 default strategy bots for a given user. Called on first user registration.
pub async fn seed_default_bots(pool: &sqlx::SqlitePool, user_id: i64) -> Result<(), sqlx::Error> {
    // Double-check: only seed if user has no bots yet
    let bot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bot_configs WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    if bot_count > 0 {
        return Ok(());
    }

    let default_bots: &[(&str, &str)] = &[
        ("Momentum", "momentum"),
        ("Mean Reversion", "mean_reversion"),
        ("Trend Follower", "trend"),
        ("Contrarian", "contrarian"),
        ("Volatility Breakout", "volatility_breakout"),
        ("Oracle Lag", "binance_signal"),
        ("Fair Value Arb", "fair_value"),
        ("T-10 Sniper", "last_seconds_scalp"),
        ("Window Delta", "window_delta"),
        ("Binance Velocity", "binance_velocity"),
        ("Smart Trend", "smart_trend"),
        ("Ultra Low Entry", "ultra_low_entry"),
        ("Trend Pullback", "trend_pullback"),
        ("Price Reversion", "price_reversion"),
        ("Sniper Value", "sniper_value"),
    ];

    for (name, strategy) in default_bots {
        sqlx::query(
            r#"
            INSERT INTO bot_configs (
                user_id, name, market_id, strategy_type, params,
                bet_size, use_kelly, kelly_fraction, max_bet, interval,
                stop_loss, take_profit, trading_mode
            ) VALUES (?, ?, 'auto', ?, '{}',
                1.0, 1, 0.25, 0.25, 60000, 0.1, 0.2, 'paper')
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(strategy)
        .execute(pool)
        .await.ok();
    }

    tracing::info!("Seeded {} default bots for user_id={}", default_bots.len(), user_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        // Run migrations to create tables
        run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_seed_default_bots_creates_15_bots() {
        let pool = create_test_pool().await;

        // Insert a test user
        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, password_hash) VALUES ('testuser', 'hash') RETURNING id"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        seed_default_bots(&pool, user_id).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bot_configs WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(count, 15);
    }

    #[tokio::test]
    async fn test_seed_default_bots_idempotent() {
        let pool = create_test_pool().await;

        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, password_hash) VALUES ('testuser', 'hash') RETURNING id"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        seed_default_bots(&pool, user_id).await.unwrap();
        seed_default_bots(&pool, user_id).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bot_configs WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(count, 15); // Still 15, not 30
    }

    #[tokio::test]
    async fn test_seed_default_bots_all_paper() {
        let pool = create_test_pool().await;

        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, password_hash) VALUES ('testuser', 'hash') RETURNING id"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        seed_default_bots(&pool, user_id).await.unwrap();

        let paper_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bot_configs WHERE user_id = ? AND trading_mode = 'paper'"
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(paper_count, 15);
    }
}

pub mod queries {
    use super::*;
    use sqlx::Row;

    pub async fn find_user_by_username(
        db: &Db,
        username: &str,
    ) -> Result<Option<(i64, String, String)>, sqlx::Error> {
        let pool = db.as_ref();
        let result = sqlx::query(
            "SELECT id, username, password_hash FROM users WHERE username = ?"
        )
        .bind(username)
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|row| {
            (
                row.get("id"),
                row.get("username"),
                row.get("password_hash"),
            )
        }))
    }

    pub async fn find_user_by_id(
        db: &Db,
        user_id: i64,
    ) -> Result<Option<(i64, String)>, sqlx::Error> {
        let result = sqlx::query(
            "SELECT id, username FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(result.map(|row| {
            (row.get("id"), row.get("username"))
        }))
    }

    pub async fn create_user(
        db: &Db,
        username: &str,
        password_hash: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO users (username, password_hash) VALUES (?, ?)"
        )
        .bind(username)
        .bind(password_hash)
        .execute(db.as_ref())
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_settings(
        db: &Db,
        user_id: i64,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        let result = sqlx::query(
            "SELECT polymarket_api_key, encrypted_blob FROM settings WHERE user_id = ?"
        )
        .bind(user_id)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(result.map(|row| {
            (
                row.get("polymarket_api_key"),
                row.get("encrypted_blob"),
            )
        }))
    }

    pub async fn upsert_settings(
        db: &Db,
        user_id: i64,
        api_key: &str,
        encrypted_blob: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO settings (user_id, polymarket_api_key, encrypted_blob)
            VALUES (?, ?, ?)
            ON CONFLICT(user_id) DO UPDATE SET
                polymarket_api_key = excluded.polymarket_api_key,
                encrypted_blob = excluded.encrypted_blob,
                updated_at = datetime('now')
            "#
        )
        .bind(user_id)
        .bind(api_key)
        .bind(encrypted_blob)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    pub async fn create_bot(
        db: &Db,
        user_id: i64,
        name: &str,
        market_id: &str,
        strategy_type: &str,
        params: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO bot_configs (user_id, name, market_id, strategy_type, params)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(user_id)
        .bind(name)
        .bind(market_id)
        .bind(strategy_type)
        .bind(params)
        .execute(db.as_ref())
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Create bot with full trading configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn create_bot_with_config(
        db: &Db,
        user_id: i64,
        name: &str,
        market_id: &str,
        strategy_type: &str,
        params: &str,
        bet_size: f64,
        use_kelly: bool,
        kelly_fraction: f64,
        max_bet: f64,
        interval: i64,
        stop_loss: f64,
        take_profit: f64,
        trading_mode: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO bot_configs (
                user_id, name, market_id, strategy_type, params,
                bet_size, use_kelly, kelly_fraction, max_bet, interval, stop_loss, take_profit, trading_mode
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(user_id)
        .bind(name)
        .bind(market_id)
        .bind(strategy_type)
        .bind(params)
        .bind(bet_size)
        .bind(use_kelly as i64)
        .bind(kelly_fraction)
        .bind(max_bet)
        .bind(interval)
        .bind(stop_loss)
        .bind(take_profit)
        .bind(trading_mode)
        .execute(db.as_ref())
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Check if a bot with the given name exists for a user (for duplicate prevention)
    pub async fn get_bot_by_name(
        db: &Db,
        user_id: i64,
        name: &str,
    ) -> Result<Option<BotRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, name, market_id, strategy_type, params, status, created_at,
                   bet_size, use_kelly, kelly_fraction, max_bet, interval, stop_loss, take_profit,
                   total_trades, winning_trades, losing_trades, win_rate, trading_mode
            FROM bot_configs
            WHERE user_id = ? AND name = ?
            "#,
        )
        .bind(user_id)
        .bind(name)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(row.map(|r| BotRecord {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            market_id: r.get("market_id"),
            strategy_type: r.get("strategy_type"),
            params: r.get("params"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            bet_size: r.get("bet_size"),
            use_kelly: r.get("use_kelly"),
            kelly_fraction: r.get("kelly_fraction"),
            max_bet: r.get("max_bet"),
            interval: r.get("interval"),
            stop_loss: r.get("stop_loss"),
            take_profit: r.get("take_profit"),
            total_trades: r.get("total_trades"),
            winning_trades: r.get("winning_trades"),
            losing_trades: r.get("losing_trades"),
            win_rate: r.get("win_rate"),
            trading_mode: r.get("trading_mode"),
        }))
    }

    pub async fn get_bots_by_user(
        db: &Db,
        user_id: i64,
    ) -> Result<Vec<BotRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, name, market_id, strategy_type, params, status, created_at,
                   bet_size, use_kelly, kelly_fraction, max_bet, interval, stop_loss, take_profit,
                   total_trades, winning_trades, losing_trades, win_rate, trading_mode
            FROM bot_configs WHERE user_id = ? ORDER BY created_at DESC
            "#
        )
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(rows.into_iter().map(|row| BotRecord {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            market_id: row.get("market_id"),
            strategy_type: row.get("strategy_type"),
            params: row.get("params"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            bet_size: row.get("bet_size"),
            use_kelly: row.get("use_kelly"),
            kelly_fraction: row.get("kelly_fraction"),
            max_bet: row.get("max_bet"),
            interval: row.get("interval"),
            stop_loss: row.get("stop_loss"),
            take_profit: row.get("take_profit"),
            total_trades: row.get("total_trades"),
            winning_trades: row.get("winning_trades"),
            losing_trades: row.get("losing_trades"),
            win_rate: row.get("win_rate"),
            trading_mode: row.get("trading_mode"),
        }).collect())
    }

    pub async fn get_bot_by_id(
        db: &Db,
        bot_id: i64,
        user_id: i64,
    ) -> Result<Option<BotRecord>, sqlx::Error> {
        let result = sqlx::query(
            r#"
            SELECT id, user_id, name, market_id, strategy_type, params, status, created_at,
                   bet_size, use_kelly, kelly_fraction, max_bet, interval, stop_loss, take_profit,
                   total_trades, winning_trades, losing_trades, win_rate, trading_mode
            FROM bot_configs WHERE id = ? AND user_id = ?
            "#
        )
        .bind(bot_id)
        .bind(user_id)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(result.map(|row| BotRecord {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            market_id: row.get("market_id"),
            strategy_type: row.get("strategy_type"),
            params: row.get("params"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            bet_size: row.get("bet_size"),
            use_kelly: row.get("use_kelly"),
            kelly_fraction: row.get("kelly_fraction"),
            max_bet: row.get("max_bet"),
            interval: row.get("interval"),
            stop_loss: row.get("stop_loss"),
            take_profit: row.get("take_profit"),
            total_trades: row.get("total_trades"),
            winning_trades: row.get("winning_trades"),
            losing_trades: row.get("losing_trades"),
            win_rate: row.get("win_rate"),
            trading_mode: row.get("trading_mode"),
        }))
    }

    pub async fn update_bot(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        name: Option<&str>,
        market_id: Option<&str>,
        strategy_type: Option<&str>,
        params: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        if let Some(n) = name {
            sqlx::query("UPDATE bot_configs SET name = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(n)
                .bind(bot_id)
                .bind(user_id)
                .execute(db.as_ref())
                .await?;
        }
        if let Some(m) = market_id {
            sqlx::query("UPDATE bot_configs SET market_id = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(m)
                .bind(bot_id)
                .bind(user_id)
                .execute(db.as_ref())
                .await?;
        }
        if let Some(s) = strategy_type {
            sqlx::query("UPDATE bot_configs SET strategy_type = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(s)
                .bind(bot_id)
                .bind(user_id)
                .execute(db.as_ref())
                .await?;
        }
        if let Some(p) = params {
            sqlx::query("UPDATE bot_configs SET params = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(p)
                .bind(bot_id)
                .bind(user_id)
                .execute(db.as_ref())
                .await?;
        }

        Ok(())
    }

    pub async fn delete_bot(
        db: &Db,
        bot_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM bot_configs WHERE id = ? AND user_id = ?")
            .bind(bot_id)
            .bind(user_id)
            .execute(db.as_ref())
            .await?;
        Ok(())
    }

    pub async fn update_bot_status(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE bot_configs SET status = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?"
        )
        .bind(status)
        .bind(bot_id)
        .bind(user_id)
        .execute(db.as_ref())
        .await?;
        Ok(())
    }

    pub async fn create_order(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        market_id: &str,
        side: &str,
        price: f64,
        size: f64,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO orders (bot_id, user_id, market_id, side, price, size)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(bot_id)
        .bind(user_id)
        .bind(market_id)
        .bind(side)
        .bind(price)
        .bind(size)
        .execute(db.as_ref())
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_orders_by_user(
        db: &Db,
        user_id: i64,
    ) -> Result<Vec<OrderRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, bot_id, market_id, side, price, size, status, order_id, created_at FROM orders WHERE user_id = ? ORDER BY created_at DESC LIMIT 100"
        )
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(rows.into_iter().map(|row| OrderRecord {
            id: row.get("id"),
            bot_id: row.get("bot_id"),
            market_id: row.get("market_id"),
            side: row.get("side"),
            price: row.get("price"),
            size: row.get("size"),
            status: row.get("status"),
            order_id: row.get("order_id"),
            created_at: row.get("created_at"),
        }).collect())
    }

    pub async fn get_positions_by_user(
        db: &Db,
        user_id: i64,
    ) -> Result<Vec<PositionRecord>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, bot_id, market_id, side, size, avg_price, current_price, pnl FROM positions WHERE user_id = ? AND size > 0"
        )
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(rows.into_iter().map(|row| PositionRecord {
            id: row.get("id"),
            bot_id: row.get("bot_id"),
            market_id: row.get("market_id"),
            side: row.get("side"),
            size: row.get("size"),
            avg_price: row.get("avg_price"),
            current_price: row.get("current_price"),
            pnl: row.get("pnl"),
        }).collect())
    }

    // === Session queries ===

    /// Create a new bot session
    /// mode: "demo" or "live"
    pub async fn create_session(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        start_balance: f64,
        strategy_config: Option<&str>,
        mode: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO bot_sessions (bot_id, user_id, start_time, start_balance, strategy_config, status, mode)
            VALUES (?, ?, datetime('now'), ?, ?, 'running', ?)
            "#
        )
        .bind(bot_id)
        .bind(user_id)
        .bind(start_balance)
        .bind(strategy_config)
        .bind(mode)
        .execute(db.as_ref())
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get active session for a bot
    pub async fn get_active_session(
        db: &Db,
        bot_id: i64,
    ) -> Result<Option<BotSessionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotSessionRecord>(
            "SELECT * FROM bot_sessions WHERE bot_id = ? AND status = 'running' ORDER BY id DESC LIMIT 1"
        )
        .bind(bot_id)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Get all running sessions (across all users) - for restoring bots on startup
    pub async fn get_all_running_sessions(
        db: &Db,
    ) -> Result<Vec<BotSessionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotSessionRecord>(
            "SELECT * FROM bot_sessions WHERE status = 'running'"
        )
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Get session by ID
    pub async fn get_session_by_id(
        db: &Db,
        session_id: i64,
    ) -> Result<Option<BotSessionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotSessionRecord>(
            "SELECT * FROM bot_sessions WHERE id = ?"
        )
        .bind(session_id)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Update session with final stats
    #[allow(clippy::too_many_arguments)]
    pub async fn end_session(
        db: &Db,
        session_id: i64,
        end_balance: f64,
        total_trades: i64,
        winning_trades: i64,
        losing_trades: i64,
        total_pnl: f64,
        max_drawdown: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE bot_sessions SET
                end_time = datetime('now'),
                end_balance = ?,
                total_trades = ?,
                winning_trades = ?,
                losing_trades = ?,
                total_pnl = ?,
                max_drawdown = ?,
                status = 'completed',
                updated_at = datetime('now')
            WHERE id = ?
            "#
        )
        .bind(end_balance)
        .bind(total_trades)
        .bind(winning_trades)
        .bind(losing_trades)
        .bind(total_pnl)
        .bind(max_drawdown)
        .bind(session_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Auto-save session (update running session)
    pub async fn update_running_session(
        db: &Db,
        session_id: i64,
        total_trades: i64,
        winning_trades: i64,
        losing_trades: i64,
        total_pnl: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE bot_sessions SET
                total_trades = ?,
                winning_trades = ?,
                losing_trades = ?,
                total_pnl = ?,
                updated_at = datetime('now')
            WHERE id = ? AND status = 'running'
            "#
        )
        .bind(total_trades)
        .bind(winning_trades)
        .bind(losing_trades)
        .bind(total_pnl)
        .bind(session_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Get session history for a bot
    pub async fn get_sessions_by_bot(
        db: &Db,
        bot_id: i64,
        limit: i64,
    ) -> Result<Vec<BotSessionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotSessionRecord>(
            "SELECT * FROM bot_sessions WHERE bot_id = ? ORDER BY start_time DESC LIMIT ?"
        )
        .bind(bot_id)
        .bind(limit)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Get all active sessions for a user
    pub async fn get_active_sessions_by_user(
        db: &Db,
        user_id: i64,
    ) -> Result<Vec<BotSessionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotSessionRecord>(
            "SELECT * FROM bot_sessions WHERE user_id = ? AND status = 'running'"
        )
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    // === Trade decision queries ===

    /// Log a trade decision
    #[allow(clippy::too_many_arguments)]
    pub async fn log_trade_decision(
        db: &Db,
        bot_id: i64,
        session_id: i64,
        user_id: i64,
        market_slug: &str,
        condition_id: &str,
        outcome: &str,
        signal_type: &str,
        signal_confidence: f64,
        btc_price: Option<f64>,
        btc_change: Option<f64>,
        market_yes_price: Option<f64>,
        market_no_price: Option<f64>,
        time_remaining: Option<i64>,
        decision_reason: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO trade_decisions (
                bot_id, session_id, user_id, market_slug, condition_id, outcome,
                signal_type, signal_confidence, btc_price, btc_change,
                market_yes_price, market_no_price, time_remaining, decision_reason
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(bot_id)
        .bind(session_id)
        .bind(user_id)
        .bind(market_slug)
        .bind(condition_id)
        .bind(outcome)
        .bind(signal_type)
        .bind(signal_confidence)
        .bind(btc_price)
        .bind(btc_change)
        .bind(market_yes_price)
        .bind(market_no_price)
        .bind(time_remaining)
        .bind(decision_reason)
        .execute(db.as_ref())
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Mark decision as executed
    pub async fn mark_decision_executed(
        db: &Db,
        decision_id: i64,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE trade_decisions SET executed = 1, order_id = ? WHERE id = ?"
        )
        .bind(order_id)
        .bind(decision_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Update decision with PnL
    pub async fn update_decision_pnl(
        db: &Db,
        decision_id: i64,
        pnl: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE trade_decisions SET pnl = ? WHERE id = ?"
        )
        .bind(pnl)
        .bind(decision_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Get trade decisions for a session
    pub async fn get_decisions_by_session(
        db: &Db,
        session_id: i64,
    ) -> Result<Vec<TradeDecisionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, TradeDecisionRecord>(
            "SELECT * FROM trade_decisions WHERE session_id = ? ORDER BY created_at DESC"
        )
        .bind(session_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Get recent decisions for a bot
    pub async fn get_recent_decisions(
        db: &Db,
        bot_id: i64,
        limit: i64,
    ) -> Result<Vec<TradeDecisionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, TradeDecisionRecord>(
            "SELECT * FROM trade_decisions WHERE bot_id = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(bot_id)
        .bind(limit)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    // === Portfolio queries ===

    /// Get portfolio for a bot (returns None if no portfolio exists)
    /// NOTE: bot_portfolios.bot_id is unique, user_id is stored for audit only
    pub async fn get_portfolio(
        db: &Db,
        bot_id: i64,
        _user_id: i64,
    ) -> Result<Option<BotPortfolioRecord>, sqlx::Error> {
        let existing = sqlx::query_as::<_, BotPortfolioRecord>(
            "SELECT * FROM bot_portfolios WHERE bot_id = ?"
        )
        .bind(bot_id)
        .fetch_optional(db.as_ref())
        .await?;

        Ok(existing)
    }

    /// Create or reset portfolio with a given starting balance
    pub async fn ensure_portfolio(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        initial_balance: f64,
    ) -> Result<BotPortfolioRecord, sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO bot_portfolios (bot_id, user_id, balance, initial_balance)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(bot_id) DO UPDATE SET
                balance = ?,
                initial_balance = ?,
                open_positions = 0,
                total_trades = 0,
                winning_trades = 0,
                losing_trades = 0,
                total_pnl = 0,
                peak_balance = ?,
                last_trade_time = NULL,
                updated_at = datetime('now')
            "#
        )
        .bind(bot_id)
        .bind(user_id)
        .bind(initial_balance)
        .bind(initial_balance)
        .bind(initial_balance)
        .bind(initial_balance)
        .execute(db.as_ref())
        .await?;

        let result = sqlx::query_as::<_, BotPortfolioRecord>(
            "SELECT * FROM bot_portfolios WHERE bot_id = ?"
        )
        .bind(bot_id)
        .fetch_one(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Update portfolio balance
    pub async fn update_portfolio_balance(
        db: &Db,
        bot_id: i64,
        balance: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE bot_portfolios SET
                balance = ?,
                peak_balance = MAX(peak_balance, ?),
                last_trade_time = datetime('now'),
                updated_at = datetime('now')
            WHERE bot_id = ?
            "#
        )
        .bind(balance)
        .bind(balance)
        .bind(bot_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Update portfolio stats after trade
    pub async fn update_portfolio_stats(
        db: &Db,
        bot_id: i64,
        total_trades: i64,
        winning_trades: i64,
        losing_trades: i64,
        total_pnl: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE bot_portfolios SET
                total_trades = ?,
                winning_trades = ?,
                losing_trades = ?,
                total_pnl = ?,
                updated_at = datetime('now')
            WHERE bot_id = ?
            "#
        )
        .bind(total_trades)
        .bind(winning_trades)
        .bind(losing_trades)
        .bind(total_pnl)
        .bind(bot_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Reset portfolio for new session
    pub async fn reset_portfolio(
        db: &Db,
        bot_id: i64,
        initial_balance: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE bot_portfolios SET
                balance = ?,
                initial_balance = ?,
                open_positions = 0,
                total_trades = 0,
                winning_trades = 0,
                losing_trades = 0,
                total_pnl = 0,
                peak_balance = ?,
                last_trade_time = NULL,
                updated_at = datetime('now')
            WHERE bot_id = ?
            "#
        )
        .bind(initial_balance)
        .bind(initial_balance)
        .bind(initial_balance)
        .bind(bot_id)
        .execute(db.as_ref())
        .await?;

        Ok(())
    }

    /// Get all portfolios for a user
    pub async fn get_portfolios_by_user(
        db: &Db,
        user_id: i64,
    ) -> Result<Vec<BotPortfolioRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotPortfolioRecord>(
            "SELECT * FROM bot_portfolios WHERE user_id = ?"
        )
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Get bot sessions with user validation
    pub async fn get_bot_sessions(
        db: &Db,
        bot_id: i64,
        user_id: i64,
    ) -> Result<Vec<BotSessionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, BotSessionRecord>(
            "SELECT * FROM bot_sessions WHERE bot_id = ? AND user_id = ? ORDER BY start_time DESC LIMIT 50"
        )
        .bind(bot_id)
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Get trade decisions for a bot with user validation
    pub async fn get_trade_decisions(
        db: &Db,
        bot_id: i64,
        user_id: i64,
    ) -> Result<Vec<TradeDecisionRecord>, sqlx::Error> {
        let result = sqlx::query_as::<_, TradeDecisionRecord>(
            "SELECT * FROM trade_decisions WHERE bot_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT 100"
        )
        .bind(bot_id)
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result)
    }

    /// Record paper trade settlement: update portfolio + trade_decisions
    /// Returns the net balance change (positive = win, negative = loss)
    pub async fn record_paper_settlement(
        db: &Db,
        bot_id: i64,
        decision_id: i64,
        won: bool,
        pnl: f64,
    ) -> Result<(), sqlx::Error> {
        let pool = db.as_ref();
        let _sign = if won { 1 } else { -1 };

        // Update trade_decision PnL
        sqlx::query("UPDATE trade_decisions SET pnl = ? WHERE id = ?")
            .bind(pnl)
            .bind(decision_id)
            .execute(pool)
            .await?;

        // Update portfolio stats
        if won {
            sqlx::query(
                r#"
                UPDATE bot_portfolios SET
                    balance = balance + ?,
                    winning_trades = winning_trades + 1,
                    total_trades = total_trades + 1,
                    total_pnl = total_pnl + ?,
                    peak_balance = MAX(peak_balance, balance + ?),
                    last_trade_time = datetime('now'),
                    updated_at = datetime('now')
                WHERE bot_id = ?
                "#,
            )
            .bind(pnl)
            .bind(pnl)
            .bind(pnl)
            .bind(bot_id)
            .execute(pool)
            .await?;
        } else {
            let loss = pnl.abs();
            sqlx::query(
                r#"
                UPDATE bot_portfolios SET
                    balance = balance - ?,
                    losing_trades = losing_trades + 1,
                    total_trades = total_trades + 1,
                    total_pnl = total_pnl - ?,
                    last_trade_time = datetime('now'),
                    updated_at = datetime('now')
                WHERE bot_id = ?
                "#,
            )
            .bind(loss)
            .bind(loss)
            .bind(bot_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Update bot-level stats (aggregated from sessions)
    pub async fn update_bot_stats(
        db: &Db,
        bot_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        // Aggregate stats from all completed sessions
        let agg: Option<(i64, i64, i64)> = sqlx::query(
            r#"
            SELECT
                SUM(total_trades) as total,
                SUM(winning_trades) as wins,
                SUM(losing_trades) as losses
            FROM bot_sessions
            WHERE bot_id = ? AND user_id = ? AND status = 'completed'
            "#
        )
        .bind(bot_id)
        .bind(user_id)
        .fetch_optional(db.as_ref())
        .await?
        .map(|row| (row.get::<i64, _>("total"), row.get::<i64, _>("wins"), row.get::<i64, _>("losses")));

        if let Some((total, wins, losses)) = agg {
            let win_rate = if total > 0 { wins as f64 / total as f64 } else { 0.0 };

            sqlx::query(
                r#"
                UPDATE bot_configs SET
                    total_trades = ?,
                    winning_trades = ?,
                    losing_trades = ?,
                    win_rate = ?,
                    updated_at = datetime('now')
                WHERE id = ? AND user_id = ?
                "#
            )
            .bind(total)
            .bind(wins)
            .bind(losses)
            .bind(win_rate)
            .bind(bot_id)
            .bind(user_id)
            .execute(db.as_ref())
            .await?;
        }

        Ok(())
    }

    /// Update bot trading config fields
    #[allow(clippy::too_many_arguments)]
    pub async fn update_bot_config(
        db: &Db,
        bot_id: i64,
        user_id: i64,
        bet_size: Option<f64>,
        use_kelly: Option<bool>,
        kelly_fraction: Option<f64>,
        max_bet: Option<f64>,
        interval: Option<i64>,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> Result<(), sqlx::Error> {
        if let Some(v) = bet_size {
            sqlx::query("UPDATE bot_configs SET bet_size = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        if let Some(v) = use_kelly {
            sqlx::query("UPDATE bot_configs SET use_kelly = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v as i64).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        if let Some(v) = kelly_fraction {
            sqlx::query("UPDATE bot_configs SET kelly_fraction = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        if let Some(v) = max_bet {
            sqlx::query("UPDATE bot_configs SET max_bet = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        if let Some(v) = interval {
            sqlx::query("UPDATE bot_configs SET interval = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        if let Some(v) = stop_loss {
            sqlx::query("UPDATE bot_configs SET stop_loss = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        if let Some(v) = take_profit {
            sqlx::query("UPDATE bot_configs SET take_profit = ?, updated_at = datetime('now') WHERE id = ? AND user_id = ?")
                .bind(v).bind(bot_id).bind(user_id).execute(db.as_ref()).await?;
        }
        Ok(())
    }

    // === API Keys CRUD ===

    pub async fn get_api_keys(
        db: &Db,
        user_id: i64,
    ) -> Result<Vec<ApiKeyRecord>, sqlx::Error> {
        let result = sqlx::query(
            "SELECT key_name, key_value, is_valid, created_at, last_validated FROM api_keys WHERE user_id = ?"
        )
        .bind(user_id)
        .fetch_all(db.as_ref())
        .await?;

        Ok(result.into_iter().map(|row| ApiKeyRecord {
            key_name: row.get("key_name"),
            key_value: row.get("key_value"),
            is_valid: row.get::<i64, _>("is_valid") != 0,
            created_at: row.get("created_at"),
            last_validated: row.get("last_validated"),
        }).collect())
    }

    pub async fn upsert_api_key(
        db: &Db,
        user_id: i64,
        key_name: &str,
        key_value: &str,
        is_valid: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO api_keys (user_id, key_name, key_value, is_valid, last_validated)
            VALUES (?, ?, ?, ?, datetime('now'))
            ON CONFLICT(user_id, key_name) DO UPDATE SET
                key_value = excluded.key_value,
                is_valid = excluded.is_valid,
                last_validated = datetime('now')
            "#,
        )
        .bind(user_id)
        .bind(key_name)
        .bind(key_value)
        .bind(is_valid as i64)
        .execute(db.as_ref())
        .await?;
        Ok(())
    }

    pub async fn delete_api_keys_by_provider(
        db: &Db,
        user_id: i64,
        provider: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM api_keys WHERE user_id = ? AND key_name LIKE ?"
        )
        .bind(user_id)
        .bind(format!("{}_%", provider))
        .execute(db.as_ref())
        .await?;
        Ok(())
    }
}

// === Data Records (structs) ===

#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    pub key_name: String,
    pub key_value: String,
    pub is_valid: bool,
    pub created_at: String,
    pub last_validated: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotRecord {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub market_id: String,
    pub strategy_type: String,
    pub params: String,
    pub status: String,
    pub created_at: String,
    // Trading configuration (may be NULL for old records)
    #[sqlx(default)]
    pub bet_size: f64,
    #[sqlx(default)]
    pub use_kelly: i64,
    #[sqlx(default)]
    pub kelly_fraction: f64,
    #[sqlx(default)]
    pub max_bet: f64,
    #[sqlx(default)]
    pub interval: i64,
    #[sqlx(default)]
    pub stop_loss: f64,
    #[sqlx(default)]
    pub take_profit: f64,
    // Stats (may be NULL for old records)
    #[sqlx(default)]
    pub total_trades: i64,
    #[sqlx(default)]
    pub winning_trades: i64,
    #[sqlx(default)]
    pub losing_trades: i64,
    #[sqlx(default)]
    pub win_rate: f64,
    #[sqlx(default)]
    pub trading_mode: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderRecord {
    pub id: i64,
    pub bot_id: i64,
    pub market_id: String,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub status: String,
    pub order_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PositionRecord {
    pub id: i64,
    pub bot_id: i64,
    pub market_id: String,
    pub side: String,
    pub size: f64,
    pub avg_price: f64,
    pub current_price: f64,
    pub pnl: f64,
}

/// Bot session record - tracking run periods
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotSessionRecord {
    pub id: i64,
    pub bot_id: i64,
    pub user_id: i64,
    pub start_time: String,
    pub end_time: Option<String>,
    pub start_balance: f64,
    pub end_balance: Option<f64>,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub total_pnl: f64,
    pub status: String,
    pub max_drawdown: f64,
    pub strategy_config: Option<String>,
    pub mode: String,
}

/// Trade decision record - why bot traded
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TradeDecisionRecord {
    pub id: i64,
    pub bot_id: i64,
    pub session_id: i64,
    pub user_id: i64,
    pub market_slug: String,
    pub condition_id: String,
    pub outcome: String,
    pub signal_type: String,
    pub signal_confidence: f64,
    pub btc_price: Option<f64>,
    pub btc_change: Option<f64>,
    pub market_yes_price: Option<f64>,
    pub market_no_price: Option<f64>,
    pub time_remaining: Option<i64>,
    pub decision_reason: Option<String>,
    pub executed: i64,
    pub order_id: Option<String>,
    pub pnl: Option<f64>,
    pub created_at: String,
}

/// Bot portfolio record - current state
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotPortfolioRecord {
    pub bot_id: i64,
    pub user_id: i64,
    pub balance: f64,
    pub initial_balance: f64,
    pub open_positions: i64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub total_pnl: f64,
    pub peak_balance: f64,
    pub last_trade_time: Option<String>,
}
