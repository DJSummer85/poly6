import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";
import type {
  Bot,
  BotRiskStatus,
  CreateBotRequest,
  LoginResponse,
  Market,
  Order,
  PlaceOrderRequest,
  PortfolioResponse,
  Position,
  RiskWarning,
  Settings,
} from "@/types";

// Auth
export function useLogin() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (credentials: { username: string; password: string }) =>
      apiFetch<LoginResponse>("/auth/login", {
        method: "POST",
        body: JSON.stringify(credentials),
      }),
    onSuccess: (data) => {
      localStorage.setItem("token", data.token);
      queryClient.invalidateQueries({ queryKey: ["user"] });
    },
  });
}

export function useUser() {
  return useQuery({
    queryKey: ["user"],
    queryFn: () => apiFetch<{ id: number; username: string }>("/auth/me"),
    enabled: typeof window !== "undefined" && !!localStorage.getItem("token"),
    retry: false,
  });
}

// Bots
export function useBots() {
  return useQuery({
    queryKey: ["bots"],
    queryFn: () => apiFetch<Bot[]>("/bots"),
    refetchInterval: 5000,
    placeholderData: keepPreviousData,
  });
}

export function useBot(id: number) {
  return useQuery({
    queryKey: ["bots", id],
    queryFn: () => apiFetch<Bot>(`/bots/${id}`),
    enabled: id > 0,
  });
}

export function useCreateBot() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreateBotRequest) =>
      apiFetch<Bot>("/bots", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
    },
  });
}

export function useStartBot() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ id, initial_balance = 100 }: { id: number; initial_balance?: number }) =>
      apiFetch<Bot>(`/bots/${id}/start`, {
        method: "POST",
        body: JSON.stringify({ initial_balance }),
      }),
    retry: 0,
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
      useAppStore.getState().updateBot(variables.id, { status: "running" });
    },
  });
}

export function useStopBot() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => apiFetch<Bot>(`/bots/${id}/stop`, { method: "POST" }),
    retry: 0,
    onSuccess: (_, id) => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
      useAppStore.getState().updateBot(id, { status: "stopped" });
    },
  });
}

export function useRunAllBots() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => apiFetch<void>("/bots/run-all", { method: "POST" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
    },
  });
}

export function useStopAllBots() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => apiFetch<void>("/bots/stop-all", { method: "POST" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
    },
  });
}

export function usePortfolio(botId: number | null) {
  return useQuery({
    queryKey: ["portfolio", botId],
    queryFn: () => apiFetch<PortfolioResponse>(`/bots/${botId}/portfolio`),
    enabled: botId !== null && botId > 0,
    refetchInterval: 5000,
  });
}

export function useAggregatePortfolio() {
  return useQuery({
    queryKey: ["aggregate-portfolio"],
    queryFn: () =>
      apiFetch<{
        total_bots: number;
        running_bots: number;
        total_balance: number;
        total_initial: number;
        total_pnl: number;
        total_trades: number;
        overall_win_rate: number;
        overall_roi_percent: number;
        avg_pnl_per_trade: number;
        unrealized_pnl: number;
        total_position_value: number;
        bots?: Array<{
          bot_id: number;
          balance: number;
          initial_balance: number;
          total_pnl: number;
          total_trades: number;
          winning_trades: number;
          losing_trades: number;
          win_rate: number;
          roi_percent: number;
          drawdown_percent: number;
          avg_pnl_per_trade: number;
          unrealized_pnl: number;
          total_position_value: number;
        }>;
      }>("/portfolio"),
    refetchInterval: 5000,
    retry: false,
  });
}

// User
export function useUserBalance() {
  return useQuery({
    queryKey: ["user-balance"],
    queryFn: () =>
      apiFetch<{ balance: number; wallet_address: string; has_credentials: boolean }>(
        "/user/balance"
      ),
    refetchInterval: 15000,
    retry: false,
  });
}

// Positions
export function usePositions() {
  return useQuery({
    queryKey: ["positions"],
    queryFn: () => apiFetch<Position[]>("/positions"),
    refetchInterval: 3000,
  });
}

export function useLivePositions() {
  return useQuery({
    queryKey: ["positions", "live"],
    queryFn: () => apiFetch<Position[]>("/positions/live"),
    refetchInterval: 5000,
  });
}

// Orders
export function useOrders() {
  return useQuery({
    queryKey: ["orders"],
    queryFn: () => apiFetch<Order[]>("/orders"),
  });
}

export function usePlaceOrder() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: PlaceOrderRequest) =>
      apiFetch<Order>("/orders", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orders"] });
      queryClient.invalidateQueries({ queryKey: ["positions"] });
    },
  });
}

export function useCancelOrder() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (orderId: string) =>
      apiFetch<void>("/orders/cancel", {
        method: "POST",
        body: JSON.stringify({ order_id: orderId }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orders"] });
    },
  });
}

// Quick Trade for UP/DOWN buttons
export function useQuickTrade() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: { side: "UP" | "DOWN"; amount: number }) =>
      apiFetch<{
        success: boolean;
        message: string;
        order_id?: string;
        btc_price?: number;
        beat_price?: number;
        error_code?: string;
      }>("/orders/quick", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orders"] });
      queryClient.invalidateQueries({ queryKey: ["positions"] });
    },
  });
}

// Markets
export function useMarkets() {
  return useQuery({
    queryKey: ["markets"],
    queryFn: () => apiFetch<Market[]>("/market/list"),
  });
}

export function useActiveMarkets() {
  return useQuery({
    queryKey: ["markets", "active"],
    queryFn: () => apiFetch<Market[]>("/market/active"),
    refetchInterval: 10000,
  });
}

export function useBtcPrice() {
  return useQuery({
    queryKey: ["btc-price"],
    queryFn: () => apiFetch<{ price: number }>("/market/btc-price"),
    refetchInterval: 1000,
  });
}

// Settings
export function useSettings() {
  return useQuery({
    queryKey: ["settings"],
    queryFn: () => apiFetch<Settings>("/settings"),
  });
}

export function useUpdateSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: Partial<Settings>) =>
      apiFetch<Settings>("/settings", {
        method: "PUT",
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useValidateCredentials() {
  return useMutation({
    mutationFn: async (data: {
      polymarket_api_key?: string;
      polymarket_api_secret?: string;
      polymarket_api_passphrase?: string;
    }) =>
      apiFetch<{ valid: boolean; balance?: number }>("/settings/validate", {
        method: "POST",
        body: JSON.stringify(data),
      }),
  });
}

// System
export function useSystemStatus() {
  return useQuery({
    queryKey: ["system-status"],
    queryFn: () =>
      apiFetch<{
        running_bots: number;
        total_bots: number;
        binance_connected: boolean;
        btc_price?: number;
        has_polymarket_credentials: boolean;
      }>("/system/status"),
    refetchInterval: 5000,
  });
}

export function useLogs() {
  return useQuery({
    queryKey: ["logs"],
    queryFn: () => apiFetch<{ logs: unknown[] }>("/system/logs"),
  });
}

// Risk Management
export function useBotRiskStatus(botId: number | null) {
  return useQuery({
    queryKey: ["risk-status", botId],
    queryFn: () => apiFetch<BotRiskStatus>(`/risk/bots/${botId}`),
    enabled: botId !== null && botId > 0,
    refetchInterval: 5000,
  });
}

export function useRiskWarnings() {
  return useQuery({
    queryKey: ["risk-warnings"],
    queryFn: () => apiFetch<RiskWarning[]>("/risk/warnings"),
    refetchInterval: 10000,
  });
}

export function usePauseBotRisk() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => apiFetch(`/risk/bots/${id}/pause`, { method: "POST" }),
    onSuccess: (_, id) => {
      queryClient.invalidateQueries({ queryKey: ["risk-status", id] });
    },
  });
}

export function useResumeBotRisk() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (id: number) => apiFetch(`/risk/bots/${id}/resume`, { method: "POST" }),
    onSuccess: (_, id) => {
      queryClient.invalidateQueries({ queryKey: ["risk-status", id] });
    },
  });
}
