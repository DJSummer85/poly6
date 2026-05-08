# Polymarket Trader

A trading bot system for Polymarket with Rust backend and Next.js frontend.

## Features

- **Multiple Trading Strategies**: Momentum, Mean Reversion, Trend Following, Contrarian, Volatility, Oracle Lag, Fair Value, Sniper, Window Delta, Binance Velocity
- **Real-time Dashboard**: Live positions, bot status, trading logs, SSE latency monitoring
- **Live Bot Activity Feed**: Real-time scanning → evaluating → trade → result event timeline
- **Strategy Performance Panel**: Win rate, P&L, and bot count per strategy type
- **Trade Feed**: Filterable live feed of all trades across all bots
- **System Health Panel**: SSE connection quality, API status, database health
- **Bot Fleet Visualization**: Compact vertical bot list with inline detail cards and activity feeds
- **Bot Management**: Create, configure, start/stop trading bots
- **Market Analysis**: Price monitoring, order books, market data
- **Order Management**: Place, track, and manage trades
- **User Authentication**: Secure login with JWT tokens
- **Encrypted Credentials**: Polymarket API keys stored encrypted in database

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Frontend (Next.js)                 │
│  ┌───────────────────────────────────────────────┐  │
│  │  CommandCenter (Dashboard)                     │  │
│  │  ├─ AccountInfoBar (balance, P&L, win rate)   │  │
│  │  ├─ MarketBar (BTC price, delta, countdown)   │  │
│  │  ├─ Trading & Chart (QuickTrade + Chart)      │  │
│  │  ├─ Bot Fleet & Positions (BotSelector+Tabs)  │  │
│  │  ├─ Market History (resolved markets)         │  │
│  │  ├─ Strategy Performance (win rate by type)   │  │
│  │  ├─ Trade Feed (live scrolling feed)          │  │
│  │  └─ System Health (SSE, API, DB status)       │  │
│  └───────────────────────────────────────────────┘  │
│                      │                               │
│              SSE (Real-time updates)                 │
└──────────────────────┼──────────────────────────────┘
                       │
┌──────────────────────┼──────────────────────────────┐
│                 Backend (Rust)                       │
│  ┌───────────────────────────────────────────────┐  │
│  │              API Layer (Axum)                  │  │
│  │  auth │ bots │ market │ orders │ settings │ sse│  │
│  └───────────────────────────────────────────────┘  │
│                      │                               │
│  ┌───────────────────────────────────────────────┐  │
│  │           Trading Engine                       │  │
│  │  PolymarketClient │ BotExecutor │ Strategies  │  │
│  │  BotOrchestrator (broadcast channel)           │  │
│  └───────────────────────────────────────────────┘  │
│                      │                               │
│  ┌───────────────────────────────────────────────┐  │
│  │           Database (SQLite)                    │  │
│  │  users │ bot_configs │ bot_sessions │ trades  │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | Next.js 15, React 19, TypeScript, Bun |
| Backend | Rust, Axum, SQLite, Tokio |
| Auth | JWT, bcrypt password hashing |
| Real-time | Server-Sent Events (SSE) |
| Crypto | AES-256-GCM for credential encryption |

## Quick Start

### Prerequisites

| Requirement | Linux/macOS | Windows |
|-------------|-------------|---------|
| **Rust** 1.70+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` | Download installer from [rustup.rs](https://rustup.rs) |
| **Bun** 1.0+ | `curl -fsSL https://bun.sh/install \| bash` | `powershell -c "irm bun.sh/install.ps1 \| iex"` |
| **Git** | `sudo apt install git` or `brew install git` | [git-scm.com](https://git-scm.com/download/win) |
| **Build tools** | `build-essential` (Linux) | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with C++ workload |

> **Note:** Bun requires Windows 10+ and works in PowerShell, CMD, or WSL. Rust's `cargo` needs the MSVC C++ compiler on Windows — install "Desktop development with C++" from Visual Studio Build Tools.

### First-Time Setup (All Platforms)

The project auto-initializes on first run — **no manual database setup or seed data needed**:

- **Database**: SQLite file is created automatically at `backend/data/polymarket_v2.db` on first backend start. All tables are created via `CREATE TABLE IF NOT EXISTS` migrations.
- **Strategies**: 20+ trading strategies are compiled into the binary. No separate installation — select any strategy when creating a bot in the UI.
- **Bots**: Created through the dashboard UI. No default bots — you configure them from scratch.
- **Credentials**: Polymarket API keys are entered via the Settings page and stored AES-256-GCM encrypted in the database.

### Setup — Linux/macOS

```bash
# Clone the repository
git clone https://github.com/Bandi86/polymarket-trader.git
cd polymarket-trader

# Create environment file
cp .env.example .env

# Edit .env and set JWT_SECRET (generate with: openssl rand -base64 32)
nano .env

# Install frontend dependencies
cd frontend && bun install && cd ..

# Start both servers (creates database automatically)
./dev.sh
```

### Setup — Windows (PowerShell)

```powershell
# Clone the repository
git clone https://github.com/Bandi86/polymarket-trader.git
cd polymarket-trader

# Create environment file (PowerShell equivalent of cp)
Copy-Item .env.example .env

# Edit .env and set JWT_SECRET
notepad .env

# Install frontend dependencies
cd frontend
bun install
cd ..

# Create database directory (optional — backend creates it automatically)
if (-not (Test-Path backend\data)) { New-Item -ItemType Directory -Path backend\data }

# Start backend in a new PowerShell window
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd $PWD\backend; cargo run --release"

# Wait a few seconds for backend to start, then start frontend
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd $PWD\frontend; bun run dev"
```

### Start Everything — One Command

**Linux/macOS:**
```bash
./dev.sh
```

**Windows (single PowerShell command — opens both servers in one window):**
```powershell
cargo run --release --manifest-path backend\Cargo.toml & bun --cwd frontend run dev
```
> Note: The `&` operator runs both commands concurrently in PowerShell. For a more robust solution, see the `dev.ps1` script below.

**Windows (alternative — `dev.ps1` helper script):**

Create `dev.ps1` in the project root:

```powershell
# dev.ps1 - Start both servers
Write-Host "Starting polymarket-trader dev environment..." -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan

# Ensure database directory exists
if (-not (Test-Path "backend\data")) {
    New-Item -ItemType Directory -Path "backend\data" | Out-Null
}

# Start backend
Write-Host "Starting backend (port 3001)..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd `"$PWD\backend`"; cargo run --release"

Start-Sleep -Seconds 3

# Start frontend
Write-Host "Starting frontend (port 3000)..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd `"$PWD\frontend`"; bun run dev"

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "Backend:  http://localhost:3001" -ForegroundColor Green
Write-Host "Frontend: http://localhost:3000" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
Write-Host "Close both PowerShell windows to stop."
```

Then run: `.\dev.ps1`

### First Time Setup

1. Open http://localhost:3000
2. Click **Register** to create an account
3. Login with your credentials
4. Go to **Settings** → Add your Polymarket API Key and Private Key
5. Go to **Bots** → Create a trading bot with your chosen strategy
6. Start the bot and monitor on **Dashboard**

### Ports

| Service | URL | Description |
|---------|-----|-------------|
| Backend | http://localhost:3001 | REST API + SSE stream |
| Frontend | http://localhost:3000 | Next.js dashboard |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/auth/register` | POST | Create new user |
| `/api/auth/login` | POST | Login, get JWT token |
| `/api/bots` | GET/POST | List/create bots |
| `/api/bots/:id/start` | POST | Start bot trading |
| `/api/bots/:id/stop` | POST | Stop bot |
| `/api/markets` | GET | List available markets |
| `/api/orders` | POST | Place order |
| `/api/settings` | GET/PUT | Manage API credentials |
| `/api/sse` | GET | Real-time event stream |

## Trading Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| **Momentum** | Follow price momentum direction | Trending markets |
| **Mean Reversion** | Bet on price returning to average | Volatile markets |
| **Trend Following** | Follow established trends | Long-term positions |
| **Contrarian** | Bet against crowd sentiment | Overreaction scenarios |
| **Volatility** | Trade based on volatility spikes | Uncertain events |
| **Oracle Lag** | Exploit price delays vs oracle | Prediction markets |
| **Fair Value** | Trade when price deviates from fair value | Mispriced markets |
| **Sniper** | Quick execution on opportunities | Fast-moving markets |
| **Window Delta** | Time-window based decisions | Scheduled events |
| **Binance Velocity** | Correlate with Binance momentum | Crypto-related markets |

## Project Structure

```
polymarket-trader/
├── backend/
│   ├── src/
│   │   ├── api/           # REST API handlers
│   │   ├── crypto/        # Encryption utilities
│   │   ├── db/            # Database queries
│   │   ├── middleware/    # Auth middleware
│   │   ├── strategies/    # Trading strategies
│   │   ├── trading/       # Polymarket client, bot executor
│   │   └── main.rs        # Entry point
│   ├── Cargo.toml
│   └── data/              # SQLite database
│
├── frontend/
│   ├── app/               # Next.js pages
│   ├── src/
│   │   ├── components/    # React components
│   │   ├── hooks/         # API hooks, SSE
│   │   ├── store/         # State management
│   │   ├── types/         # TypeScript types
│   │   └── lib/           # Utilities
│   ├── package.json
│   └── bun.lock
│
├── docs/                  # Documentation
├── .env.example           # Environment template
├── dev.sh                 # Development runner (Linux/macOS)
├── dev.ps1                # Development runner (Windows PowerShell)
└── README.md
```

## Security

- **Credentials encrypted**: Polymarket API keys stored with AES-256-GCM
- **JWT authentication**: Session tokens with 24h expiry
- **Password hashing**: bcrypt with cost factor 12
- **No secrets in code**: All credentials via environment or UI

## License

MIT

## Contributing

PRs welcome. Please run tests before submitting:

```bash
cd backend && cargo test
cd frontend && bun test
```