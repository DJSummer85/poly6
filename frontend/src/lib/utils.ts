import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatPrice(price: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(price);
}

export function formatPercent(value: number): string {
  const sign = value >= 0 ? "+" : "";
  return `${sign}${value.toFixed(2)}%`;
}

export function formatTime(date: Date | number): string {
  const d = typeof date === "number" ? new Date(date) : date;
  return d.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}s`;
}

// ── Strategy type colors (used in bot-row, bot-detail-card) ──

export const STRATEGY_COLORS: Record<string, string> = {
  momentum: "#8b5cf6",
  mean_reversion: "#06b6d4",
  last_seconds_scalp: "#f59e0b",
  binance_signal: "#22c55e",
  contrarian: "#ec4899",
  smart_trend: "#3b82f6",
  default: "#71717a",
};

export function getStrategyColor(strategy: string): string {
  return STRATEGY_COLORS[strategy] || STRATEGY_COLORS.default;
}

export function strategyAbbr(s: string): string {
  if (s === "last_seconds_scalp") return "LSS";
  if (s === "mean_reversion") return "MR";
  if (s === "binance_signal") return "BS";
  return s.substring(0, 3).toUpperCase();
}

// API base URL - use environment variable or default to localhost backend
const getApiBase = (): string => {
  // In browser, we can use relative paths with Next.js API routes
  // But for direct backend calls, we need the full URL
  if (typeof window !== "undefined") {
    // Check if we're running with Next.js dev server (proxy) or direct
    const isDev = window.location.port === "3000";
    if (isDev) {
      // Direct call to backend on port 3001
      return "http://localhost:3001/api";
    }
    // Production - use same origin with /api prefix
    return "/api";
  }
  // Server-side default
  return process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001/api";
};

const API_BASE = getApiBase();

export async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const token = typeof window !== "undefined" ? localStorage.getItem("token") : null;
  const headers: HeadersInit = {
    "Content-Type": "application/json",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...options?.headers,
  };

  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ message: "Unknown error" }));
    const msg = error.message || error.error || "Unknown error";
    if (response.status === 401) {
      throw new Error("Nincs jogosultsága a művelethez. Kérjük, jelentkezzen be újra.");
    } else if (response.status === 403) {
      throw new Error("Hozzáférés megtagadva.");
    } else if (response.status === 404) {
      throw new Error(`Az erőforrás nem található: ${path}`);
    } else if (response.status === 502 || response.status === 503) {
      throw new Error("A szerver nem érhető el. Kérjük, ellenőrizze a backend futását.");
    } else {
      throw new Error(msg);
    }
  }

  return response.json();
}
