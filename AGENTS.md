# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

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
- **State** (`src/store/`): Zustand for global state (prices, bots, positions)
- Uses Bun, not npm/yarn

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
  "sentiment": "UP"           // "UP" if yes > 0.5
}
```

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
| `backend/src/api/sse.rs` | SSE stream for real-time Polymarket data |
| `backend/src/trading/orchestrator.rs` | Bot lifecycle management |
| `frontend/src/hooks/use-sse.ts` | SSE connection and data parsing |
| `frontend/src/store/index.ts` | Global state (prices, bots, positions) |
| `frontend/src/components/dashboard/compact-data-bar.tsx` | Main dashboard display |

## Environment

Required in `.env`:
- `JWT_SECRET`: Generate with `openssl rand -base64 32`

Database auto-created at `backend/data/polymarket_v2.db`.

## Notes

- Frontend uses Bun (not npm) - run `bun install`, `bun run dev`
- Backend binary name: `polymarket-v2` (not `polymarket-v2-backend`)
- SSE events: `market`, `status`, `connected`
- Frontend port 3000, Backend port 3001 (CORS enabled)