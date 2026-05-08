# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Polymarket trading bot system with Rust backend and Next.js frontend. Trades on Polymarket BTC Up/Down 5-minute markets.

## Commands

### Development
```bash
# Run both servers (backend + frontend)
./dev.sh

# Or run separately:
cd backend && cargo run --release          # Backend on :3001
cd frontend && bun run dev                  # Frontend on :3000
```

### Build
```bash
cargo build --release --manifest-path backend/Cargo.toml   # Backend
cd frontend && bun run build                               # Frontend
```

### Lint/Format
```bash
cd frontend && bun run lint           # Biome check
cd frontend && bun run lint:fix       # Biome fix
```

### Test
```bash
cd backend && cargo test              # Rust tests
```

## Architecture

### Backend (Rust)
- **API Layer** (`src/api/`): Axum handlers for auth, bots, orders, settings, SSE
- **Trading Engine** (`src/trading/`): Polymarket client, bot executor, orchestrator
- **Strategies** (`src/strategies/`): Momentum, Mean Reversion, Trend, Contrarian, etc.
- **Database**: SQLite via sqlx, tables: users, bot_configs, bot_sessions, trades
- **SSE** (`src/api/sse.rs`): Real-time Polymarket data at 300ms intervals

### Frontend (Next.js)
- **App Router** (`app/`): Pages for dashboard, bots, markets, orders, settings
- **Components** (`src/components/`): dashboard, layout, ui
- **Hooks** (`src/hooks/`): use-api.ts (REST), use-sse.ts (real-time)
- **State** (`src/store/`): Zustand for global state (prices, bots, positions, latency, bot activities)
- Uses Bun, not npm/yarn

### Dashboard Architecture

The CommandCenter (`command-center.tsx`) organizes the dashboard into collapsible panels:
1. **AccountInfoBar** — Balance, aggregate P&L, win rate, latency sparkline
2. **MarketBar** — BTC price, target, delta, volume, time-to-resolution progress
3. **Trading & Chart** — QuickTradePanel + ChartPanel (side by side)
4. **Bot Fleet & Positions** — BotSelector + ActivityTabs (side by side)
5. **Market History** — Resolved market results table
6. **Strategy Performance** — Per-strategy win rate, P&L, bot count
7. **Trade Feed** — Live scrolling feed with filter (ALL/UP/DOWN/WIN/LOSS)
8. **System Health** — SSE latency chart, API/DB status, bot fleet summary

Panel visibility is stored in Zustand (`panels` state) with persist middleware.

### Key Dashboard Components

| File | Purpose |
|------|---------|
| `command-center.tsx` | Main dashboard layout, AccountInfoBar, MarketBar |
| `bot-selector.tsx` | Vertical bot list with selection |
| `bot-row.tsx` | Compact bot row display |
| `bot-detail-card.tsx` | Detailed bot card with portfolio + activity |
| `live-bot-activity-card.tsx` | Real-time bot activity timeline |
| `strategy-performance.tsx` | Per-strategy analytics |
| `trade-feed.tsx` | Live trade feed with filters |
| `system-health.tsx` | System status monitoring |
| `compact-data-bar.tsx` | Market data bar |
| `quick-trade-panel.tsx` | Manual UP/DOWN trading |
| `collapsible-panel.tsx` | Reusable expandable section |

### Real-time Data Flow
```
Polymarket CLOB API → Backend SSE → Frontend useSSE → Zustand store → UI
```
- SSE endpoint: `/api/events` (backend port 3001)
- Market events every 300ms with: btc_price, start_price, yes/no odds, time_remaining
- Frontend connects to `http://localhost:3001/api/events` in dev mode

## Key Patterns

### Polymarket BTC Up/Down Markets
- Markets use timestamp-based slugs: `btc-updown-5m-{unix_timestamp}` (5-min intervals)
- Resolution: BTC price at END vs START of 5-minute window
- **Price to beat** = BTC price at market start time
- YES wins if BTC ≥ target at end; NO wins if BTC < target
- Backend captures start_price when market begins

### SSE Data Structure
```json
{
  "btc_price": 78000.0,
  "start_price": 77998.0,     // Target price (captured at market start)
  "price_delta": 2.0,         // Current - Start
  "yes": 0.525,               // YES probability (0-1)
  "no": 0.475,                // NO probability (0-1)
  "time_remaining": 250,      // Seconds until market closes
  "volume": 150000.0,         // Market volume/liquidity
  "sentiment": "UP",          // "UP" if yes > 0.5
  "event_start_time": 1714190400,  // Market start Unix timestamp
  "server_timestamp": 1714190650123, // Server time in ms (for latency calc)
  "seq": 42                   // Monotonically increasing sequence number
}
```

### Bot Activity SSE Events

The orchestrator broadcasts these event types via `bot_event_broadcaster`:

| Type | Fields | Description |
|------|--------|-------------|
| `session_started` | bot_id, session_id, bot_name | Bot trading session began |
| `session_ended` | bot_id, session_id, final_balance, total_pnl | Bot session ended |
| `trade_decision` | bot_id, outcome, confidence, bet_size, reason | Strategy decided to trade |
| `order_executed` | bot_id, order_id | Order placed on CLOB |
| `balance_updated` | bot_id, balance | Wallet balance changed |
| `error` | bot_id, message | Bot encountered error |
| `market_transition` | new_market_slug | Market rolled over |
| `scanning` | bot_id, market_slug | Bot scanning for opportunities |
| `evaluating` | bot_id, strategy, confidence | Strategy evaluation in progress |
| `position_update` | bot_id, side, size, price, unrealized_pnl | Position changed |
| `trade_result` | bot_id, won, pnl | Trade settled (win/loss) |

### API Credentials
- Polymarket API keys stored **encrypted** in database (AES-256-GCM)
- Set via Settings UI, NOT in .env
- Backend decrypts on-demand for trading operations

### Frontend SSE Connection
```typescript
// use-sse.ts connects to backend SSE
const isDev = window.location.port === "3000";
const baseUrl = isDev ? "http://localhost:3001" : window.location.origin;
const eventSource = new EventSource(`${baseUrl}/api/events`);
```

## Important Files

| File | Purpose |
|------|---------|
| `backend/src/api/sse.rs` | SSE stream for real-time Polymarket data (market + bot events) |
| `backend/src/trading/orchestrator.rs` | Bot lifecycle management, event broadcasting |
| `frontend/src/hooks/use-sse.ts` | SSE connection, data parsing, latency calculation |
| `frontend/src/hooks/use-api.ts` | React Query hooks for REST API |
| `frontend/src/store/index.ts` | Global state (prices, bots, positions, latency, bot activities) |
| `frontend/src/components/dashboard/command-center.tsx` | Main dashboard layout |
| `frontend/src/components/dashboard/live-bot-activity-card.tsx` | Real-time bot activity timeline |
| `frontend/src/components/dashboard/strategy-performance.tsx` | Strategy analytics panel |
| `frontend/src/components/dashboard/trade-feed.tsx` | Live trade feed with filters |
| `frontend/src/components/dashboard/system-health.tsx` | System status monitoring |

## Environment

Required in `.env`:
- `JWT_SECRET`: Generate with `openssl rand -base64 32`

Database auto-created at `backend/data/polymarket_v2.db`.

## Notes

- Frontend uses Bun (not npm) - run `bun install`, `bun run dev`
- Backend binary name: `polymarket-v2` (not `polymarket-v2-backend`)
- SSE events: `market`, `status`, `connected`
- Frontend port 3000, Backend port 3001 (CORS enabled)