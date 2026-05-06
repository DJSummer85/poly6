import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { Bot, BotLogEvent, Market, Position, SystemStatus } from "@/types";

interface AppState {
  // Auth
  token: string | null;
  user: { id: number; email: string; username: string } | null;
  isAuthenticated: boolean;
  setAuth: (token: string, user: { id: number; email: string; username: string }) => void;
  clearAuth: () => void;

  // Trading Mode
  tradingMode: "demo" | "live";
  setTradingMode: (mode: "demo" | "live") => void;

  // Bots
  bots: Bot[];
  selectedBotIds: number[];
  setBots: (bots: Bot[]) => void;
  setSelectedBotIds: (ids: number[]) => void;
  addSelectedBot: (id: number) => void;
  removeSelectedBot: (id: number) => void;
  updateBot: (id: number, updates: Partial<Bot>) => void;

  // Market Data
  btcPrice: number;
  startPrice: number;
  priceDelta: number;
  beatPrice: number;
  yesPrice: number;
  noPrice: number;
  marketQuestion: string;
  currentMarket: Market | null;
  timeRemaining: number;
  volume: number;
  apiLatency: number;
  setMarketData: (data: {
    btcPrice?: number;
    startPrice?: number;
    priceDelta?: number;
    beatPrice?: number;
    yesPrice?: number;
    noPrice?: number;
    marketQuestion?: string;
    timeRemaining?: number;
    volume?: number;
    apiLatency?: number;
  }) => void;

  // Market History
  marketHistory: {
    endTime: number;
    targetPrice: number;
    finalPrice: number;
    delta: number;
    duration: number;
  }[];
  setMarketHistory: (history: AppState["marketHistory"]) => void;
  addMarketResult: (result: AppState["marketHistory"][0]) => void;

  // Balance & Credentials
  userBalance: number | null;
  hasCredentials: boolean;
  setUserBalance: (balance: number | null) => void;
  setHasCredentials: (has: boolean) => void;

  // Positions
  positions: Position[];
  setPositions: (positions: Position[]) => void;

  // Logs
  logs: BotLogEvent[];
  addLog: (log: BotLogEvent) => void;
  clearLogs: () => void;

  // System Status
  systemStatus: SystemStatus | null;
  setSystemStatus: (status: SystemStatus) => void;

  // SSE Latency Tracking
  latency: {
    current: number;
    avg: number;
    min: number;
    max: number;
    samples: number[];
  };
  setLatency: (latencyMs: number) => void;

  // Bot Activity Feed
  botActivities: Record<
    number,
    Array<{
      id: string;
      botId: number;
      type: string;
      timestamp: number;
      data: Record<string, unknown>;
    }>
  >;
  addBotActivity: (
    botId: number,
    activity: Omit<AppState["botActivities"][number][0], "id">
  ) => void;
  clearBotActivities: (botId: number) => void;

  // UI
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  emergencyStopActive: boolean;
  setEmergencyStop: (active: boolean) => void;

  // Dashboard Panels
  panels: {
    marketData: boolean;
    tradeAndChart: boolean;
    botsAndPositions: boolean;
    history: boolean;
    strategyPerformance: boolean;
    tradeFeed: boolean;
    systemHealth: boolean;
  };
  togglePanel: (panel: keyof AppState["panels"]) => void;
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      // Auth
      token: null,
      user: null,
      isAuthenticated: false,
      setAuth: (token, user) => {
        if (typeof window !== "undefined") {
          localStorage.setItem("token", token);
        }
        set({ token, user, isAuthenticated: true });
      },
      clearAuth: () => {
        if (typeof window !== "undefined") {
          localStorage.removeItem("token");
        }
        set({ token: null, user: null, isAuthenticated: false });
      },

      // Trading Mode
      tradingMode: "demo",
      setTradingMode: (mode) => set({ tradingMode: mode }),

      // Bots
      bots: [],
      selectedBotIds: [],
      setBots: (bots) => set({ bots }),
      setSelectedBotIds: (ids) => set({ selectedBotIds: ids }),
      addSelectedBot: (id) =>
        set((state) => {
          if (state.selectedBotIds.length >= 2 || state.selectedBotIds.includes(id)) return state;
          return { selectedBotIds: [...state.selectedBotIds, id] };
        }),
      removeSelectedBot: (id) =>
        set((state) => ({
          selectedBotIds: state.selectedBotIds.filter((bid) => bid !== id),
        })),
      updateBot: (id, updates) =>
        set((state) => ({
          bots: state.bots.map((bot) => (bot.id === id ? { ...bot, ...updates } : bot)),
        })),

      // Market Data
      btcPrice: 0,
      startPrice: 0,
      priceDelta: 0,
      beatPrice: 0,
      yesPrice: 0.5,
      noPrice: 0.5,
      marketQuestion: "",
      currentMarket: null,
      timeRemaining: 0,
      volume: 0,
      apiLatency: 0,
      setMarketData: (data) =>
        set({
          ...(data.btcPrice !== undefined && { btcPrice: data.btcPrice }),
          ...(data.startPrice !== undefined && { startPrice: data.startPrice }),
          ...(data.priceDelta !== undefined && { priceDelta: data.priceDelta }),
          ...(data.beatPrice !== undefined && { beatPrice: data.beatPrice }),
          ...(data.yesPrice !== undefined && { yesPrice: data.yesPrice }),
          ...(data.noPrice !== undefined && { noPrice: data.noPrice }),
          ...(data.marketQuestion !== undefined && { marketQuestion: data.marketQuestion }),
          ...(data.timeRemaining !== undefined && { timeRemaining: data.timeRemaining }),
          ...(data.volume !== undefined && { volume: data.volume }),
          ...(data.apiLatency !== undefined && { apiLatency: data.apiLatency }),
        }),

      // Market History
      marketHistory: [],
      setMarketHistory: (history) => set({ marketHistory: history }),
      addMarketResult: (result) =>
        set((state) => ({
          marketHistory: [...state.marketHistory.slice(-4), result],
        })),

      // Balance & Credentials
      userBalance: null,
      hasCredentials: false,
      setUserBalance: (balance) => set({ userBalance: balance }),
      setHasCredentials: (has) => set({ hasCredentials: has }),

      // Positions
      positions: [],
      setPositions: (positions) => set({ positions }),

      // Logs
      logs: [],
      addLog: (log) =>
        set((state) => ({
          logs: [...state.logs.slice(-100), log],
        })),
      clearLogs: () => set({ logs: [] }),

      // System Status
      systemStatus: null,
      setSystemStatus: (status) => set({ systemStatus: status }),

      // SSE Latency Tracking
      latency: { current: 0, avg: 0, min: 0, max: 0, samples: [] },
      setLatency: (latencyMs) =>
        set((state) => {
          const samples = [...state.latency.samples, latencyMs].slice(-100);
          const sum = samples.reduce((a, b) => a + b, 0);
          const currentMin = state.latency.samples.length === 0 ? latencyMs : state.latency.min;
          return {
            latency: {
              current: latencyMs,
              avg: Math.round(sum / samples.length),
              min: Math.min(currentMin, latencyMs),
              max: Math.max(state.latency.max, latencyMs),
              samples,
            },
          };
        }),

      // Bot Activity Feed
      botActivities: {},
      addBotActivity: (botId, activity) =>
        set((state) => {
          const existing = state.botActivities[botId] ?? [];
          return {
            botActivities: {
              ...state.botActivities,
              [botId]: [
                ...existing.slice(-19),
                { ...activity, id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}` },
              ],
            },
          };
        }),
      clearBotActivities: (botId) =>
        set((state) => ({
          botActivities: { ...state.botActivities, [botId]: [] },
        })),

      // UI
      sidebarCollapsed: false,
      toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
      emergencyStopActive: false,
      setEmergencyStop: (active) => set({ emergencyStopActive: active }),

      // Dashboard Panels
      panels: {
        marketData: true,
        tradeAndChart: true,
        botsAndPositions: true,
        history: true,
        strategyPerformance: true,
        tradeFeed: true,
        systemHealth: false,
      },
      togglePanel: (panel) =>
        set((state) => ({
          panels: {
            ...state.panels,
            [panel]: !state.panels[panel],
          },
        })),
    }),
    {
      name: "polytrade-auth",
      partialize: (state) => ({
        token: state.token,
        user: state.user,
        isAuthenticated: state.isAuthenticated,
        panels: state.panels,
        tradingMode: state.tradingMode,
      }),
    }
  )
);

export const createSSEConnection = (onMessage: (event: MessageEvent) => void): EventSource => {
  const token = localStorage.getItem("token");
  const isDev = typeof window !== "undefined" && window.location.port === "3000";
  const baseUrl = isDev ? "http://localhost:3001" : window.location.origin;
  const url = new URL(`${baseUrl}/api/events`);
  if (token) {
    url.searchParams.set("token", token);
  }
  const eventSource = new EventSource(url.toString());
  eventSource.onmessage = onMessage;
  eventSource.onerror = (e) => {
    console.error("SSE connection error:", e);
  };
  return eventSource;
};
