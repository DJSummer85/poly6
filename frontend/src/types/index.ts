// API Types matching Rust backend

export interface User {
  id: number;
  email: string;
  username: string;
  created_at: string;
}

export interface Bot {
  id: number;
  name: string;
  market_id: string;
  strategy: StrategyType;
  strategy_type: string;
  trading_mode: string;
  bet_size: number;
  max_bet: number;
  use_kelly: boolean;
  kelly_fraction: number;
  interval: number;
  interval_seconds: number;
  volatility_threshold?: number;
  btc_confirmation?: boolean;
  entry_bounds?: number[];
  status: BotStatus;
  run_time?: number;
  created_at: string;
  pnl?: number;
  trades_count?: number;
  win_rate?: number;
  stop_loss: number;
  take_profit: number;
}

export type StrategyType =
  | "window_delta"
  | "fair_value"
  | "last_seconds_scalp"
  | "momentum"
  | "binance_signal"
  | "contrarian"
  | "smart_trend"
  | "ultra_low_entry"
  | "volatility_breakout"
  | "trend_pullback"
  | "price_reversion"
  | "binance_velocity"
  | "sniper_value"
  | "odds_swing"
  | "bayesian_ev"
  | "mean_reversion"
  | "trend"
  | "volatility"
  | "random";

export const STRATEGY_LABELS: Record<
  StrategyType,
  { name: string; category: string; description: string }
> = {
  window_delta: {
    name: "Window Delta",
    category: "Momentum",
    description: "BTC ár vs ablak nyitóár alapján",
  },
  fair_value: {
    name: "Fair Value Arb",
    category: "Arbitrage",
    description: "Piac félreárazást keres",
  },
  last_seconds_scalp: {
    name: "T-10 Sniper",
    category: "Arbitrage",
    description: "Utolsó 10-30mp-ban lép",
  },
  momentum: {
    name: "BTC Momentum",
    category: "Momentum",
    description: "BTC momentum alapú kereskedés",
  },
  binance_signal: {
    name: "Oracle Lag",
    category: "Momentum",
    description: "Binance valós idejű BTC ár előnye",
  },
  contrarian: {
    name: "Contrarian",
    category: "Mean Rev",
    description: "Piac követ – nem igazi contrarian",
  },
  smart_trend: {
    name: "Smart Trend",
    category: "Trend",
    description: "Multi-timeframe trend + BTC megerősítés",
  },
  ultra_low_entry: {
    name: "Ultra Low Entry",
    category: "Mean Rev",
    description: "Vásárlás 4-15¢-nél",
  },
  volatility_breakout: {
    name: "Vol Breakout",
    category: "Momentum",
    description: "Csak extrém volatilitásnál kereskedik",
  },
  trend_pullback: {
    name: "Trend Pullback",
    category: "Momentum",
    description: "Magas meggyőzési órákon kereskedik",
  },
  price_reversion: {
    name: "Price Reversion",
    category: "Mean Rev",
    description: "Polymarket ár visszatérés",
  },
  binance_velocity: {
    name: "Binance Velocity",
    category: "Momentum",
    description: "BTC sebesség alapú",
  },
  sniper_value: {
    name: "Sniper Value",
    category: "Mean Rev",
    description: "Extremális áraknál kereskedik",
  },
  odds_swing: {
    name: "Odds Swing",
    category: "Other",
    description: "Vásárol <15¢ alatt, kilép 2x-nél",
  },
  bayesian_ev: {
    name: "Bayesian EV",
    category: "Arbitrage",
    description: "Bayesian valószínűség + EV szűrő + Kelly",
  },
  mean_reversion: {
    name: "Mean Reversion",
    category: "Mean Rev",
    description: "Extrém elmozdulás után visszatérés",
  },
  trend: { name: "Multi-level Trend", category: "Trend", description: "Trend követés" },
  volatility: { name: "Volatility", category: "Momentum", description: "Volatilitás kitörés" },
  random: { name: "Random", category: "Other", description: "Véletlen kereskedés" },
};

export type BotStatus = "idle" | "running" | "stopped" | "error";

export interface BotSession {
  id: number;
  bot_id: number;
  start_time: number;
  end_time?: number;
  start_balance: number;
  end_balance?: number;
  total_trades: number;
  winning_trades: number;
  status: "active" | "completed" | "stopped";
}

export interface BotStats {
  trades: number;
  wins: number;
  losses: number;
  pnl: number;
  win_rate: number;
  ev: number;
  sharpe?: number;
}

export interface Portfolio {
  balance: number;
  usdc_balance?: number;
  matic_balance?: number;
  positions: Position[];
  closed_positions: Position[];
}

export interface PortfolioResponse {
  bot_id: number;
  balance: number;
  initial_balance: number;
  open_positions: number;
  total_trades: number;
  winning_trades: number;
  losing_trades: number;
  total_pnl: number;
  peak_balance: number;
  win_rate: number;
  roi_percent: number;
  drawdown_percent: number;
  avg_pnl_per_trade: number;
  unrealized_pnl: number;
  total_position_value: number;
}

export interface Position {
  id: number;
  market_id: string;
  outcome: "YES" | "NO";
  amount: number;
  odds: number;
  stake: number;
  fee: number;
  timestamp: number;
  status: "open" | "closed" | "settled";
  pnl?: number;
  bot_id?: number;
}

export interface Order {
  id: string;
  market_id: string;
  outcome: "YES" | "NO";
  side: "BUY" | "SELL";
  price: number;
  size: number;
  status: "PENDING" | "FILLED" | "CANCELLED";
  created_at: number;
  filled_at?: number;
  bot_id?: number;
}

export interface Market {
  id: string;
  question: string;
  outcomes: ["YES", "NO"];
  outcome_prices: [number, number];
  volume: number;
  active: boolean;
  expires_at?: number;
}

export interface Settings {
  polymarket_api_key?: string;
  polymarket_api_secret?: string;
  polymarket_api_passphrase?: string;
  binance_api_key?: string;
  binance_api_secret?: string;
  daily_loss_limit?: number;
  emergency_stop_enabled?: boolean;
  has_credentials?: boolean;
  wallet_address?: string;
  balance?: string;
}

export interface SystemStatus {
  bots_running: number;
  bots_total: number;
  total_pnl: number;
  active_positions: number;
  binance_connected: boolean;
  last_update: number;
  // From /system/status endpoint
  running_bots?: number;
  total_bots?: number;
  has_polymarket_credentials?: boolean;
  polymarket_api_key?: string;
  btc_price?: number;
}

// SSE Event Types
export interface SSEEvent {
  type: "connected" | "market" | "bot" | "bot_log" | "position" | "status";
  data: unknown;
}

export interface BotLogEvent {
  bot_id: number;
  bot_name: string;
  message: string;
  timestamp: number;
  level: "info" | "warn" | "error" | "success";
}

export interface MarketUpdateEvent {
  btc_price: number;
  beat_price: number;
  current_market?: Market;
  time_remaining?: number;
}

// API Response Types
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export interface LoginResponse {
  token: string;
  user: User;
}

export interface CreateBotRequest {
  name: string;
  strategy: StrategyType;
  bet_size: number;
  max_bet?: number;
  use_kelly?: boolean;
  kelly_fraction?: number;
  interval_seconds?: number;
  volatility_threshold?: number;
  btc_confirmation?: boolean;
}

export interface PlaceOrderRequest {
  market_id: string;
  outcome: "YES" | "NO";
  side: "BUY" | "SELL";
  price: number;
  size: number;
  bot_id?: number;
}

// Risk Management Types
export interface RiskWarning {
  bot_id: number;
  warning_type: string;
  message: string;
  severity: "warning" | "critical";
  timestamp: number;
}

export interface BotRiskStatus {
  bot_id: number;
  current_drawdown: number;
  daily_pnl: number;
  trades_today: number;
  paused: boolean;
  pause_reason: string | null;
  warnings: RiskWarning[];
  actions: string[];
}
