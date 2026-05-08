# Polymarket Trading bot v2 - Starting Plan

## Architecture Decision (2026-04-21)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Backend | Rust + Axum HTTP API | Simple, debuggable, good performance |
| Auth | Username/password (local) | SQLite-backed, encrypted passwords |
| Wallet | UI input + encrypted storage | Keys stored encrypted in SQLite |
| Scope | BTC 5min only | Focus on single market first |

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Next.js Frontend                        │
│  (Dashboard, Bot Config, Order Book, Trade History, Settings)  │
└─────────────────────────────────────────────────────────────────┘
                              │ HTTP + WebSocket
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Rust Backend (Axum)                        │
├─────────────────────────────────────────────────────────────────┤
│  /auth    │ /bot    │ /orders   │ /positions   │ /settings    │
│           │         │           │              │              │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    Trading Engine                           │ │
│  │  - Order Book WebSocket Client                             │ │
│  │  - Strategy Executor (BTC 5min)                             │ │
│  │  - Position Manager                                         │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              Database (SQLite)                              │ │
│  │  - Users, Bots, Orders, Positions, Settings                │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    External APIs                                │
│  - Polymarket CLOB API (orders, trades)                        │
│  - Polymarket Gamma API (market data)                           │
│  - Polygon RPC (on-chain, optional)                            │
└─────────────────────────────────────────────────────────────────┘
```

---

## API Endpoints Design

### Auth
- `POST /api/auth/register` - Create local account
- `POST /api/auth/login` - Login, returns JWT
- `GET /api/auth/me` - Get current user

### Bot Management
- `POST /api/bots` - Create bot config
- `GET /api/bots` - List all bots
- `GET /api/bots/:id` - Get bot status
- `POST /api/bots/:id/start` - Start bot
- `POST /api/bots/:id/stop` - Stop bot
- `PUT /api/bots/:id` - Update bot config
- `DELETE /api/bots/:id` - Delete bot

### Trading
- `GET /api/orders` - Get order history
- `GET /api/positions` - Get current positions
- `GET /api/markets` - List available markets
- `GET /api/orderbook/:market` - Get order book

### Settings
- `GET /api/settings` - Get user settings (API keys, etc.)
- `PUT /api/settings` - Update settings (encrypted storage)

### WebSocket
- `WS /ws/orders/:botId` - Real-time order updates
- `WS /ws/positions` - Real-time position updates

---

## Database Schema

### users
- id, username, password_hash, created_at, updated_at

### bot_configs
- id, user_id, name, market_id, strategy_type, params (JSON), created_at

### orders
- id, bot_id, market_id, side, price, size, status, filled_at, created_at

### positions
- id, bot_id, market_id, side, size, avg_price, current_price, pnl

### settings (encrypted)
- id, user_id, polymarket_api_key, polymarket_private_key, encrypted_blob

---

## Key Implementation Details

### 1. Private Key Storage
- User enters private key in UI
- Encrypted with AES-256-GCM using user password as key
- Stored in SQLite as encrypted blob
- Never sent to any external service

### 2. Trading Flow
1. Bot subscribes to order book via WebSocket
2. Strategy evaluates signals
3. When signal triggers → POST order to CLOB API
4. Order response → store in DB, emit WebSocket event

### 3. Fee Handling
- Fetch current `feeRateBps` from CLOB API
- Include in order signature
- Validate before submitting

---

## Phase 1: Core Infrastructure

1. **Setup Rust project** with Axum, SQLite, WebSocket
2. **Database schema** - migrations
3. **Auth system** - register, login, JWT middleware
4. **Settings API** - encrypted key storage
5. **Basic health check** endpoints

## Phase 2: Trading Engine

1. **WebSocket client** - Polymarket order book
2. **CLOB API client** - place orders, get order status
3. **Strategy executor** - BTC 5min signal evaluation
4. **Position tracking** - real-time P&L

## Phase 3: Frontend

1. **Dashboard** - bot status, quick actions
2. **Bot config** - create/edit bot settings
3. **Order book** - real-time display
4. **Trade history** - past trades
5. **Settings** - API key input

## Phase 4: Polish

1. **Error handling** - reconnection logic, rate limiting
2. **Logging** - structured logging
3. **Testing** - integration tests
4. **Deployment** - local first

---

## Code Structure

```
polymarket-v2/
├── backend/
│   ├── src/
│   │   ├── main.rs
│   │   ├── api/
│   │   │   ├── auth.rs
│   │   │   ├── bots.rs
│   │   │   ├── orders.rs
│   │   │   ├── positions.rs
│   │   │   └── settings.rs
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   └── migrations/
│   │   ├── trading/
│   │   │   ├── mod.rs
│   │   │   ├── client.rs      # CLOB API client
│   │   │   ├── websocket.rs   # Order book WS
│   │   │   ├── strategy.rs   # Strategy executor
│   │   │   └── executor.rs   # Order placement
│   │   ├── crypto/
│   │   │   └── encryption.rs # AES-256-GCM
│   │   └── middleware/
│   │       └── auth.rs
│   ├── Cargo.toml
│   └── .env.example
├── frontend/
│   ├── src/
│   │   ├── app/
│   │   ├── components/
│   │   ├── lib/
│   │   └── types/
│   ├── package.json
│   └── .env.local
└── README.md
```

---

## Security Considerations

- Private keys encrypted at rest
- JWT tokens with short expiry
- Rate limiting on all endpoints
- No sensitive data in logs
- HTTPS in production