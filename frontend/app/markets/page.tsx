"use client";

import { AnimatePresence, motion } from "framer-motion";
import {
  Activity,
  BarChart3,
  Clock,
  DollarSign,
  Search,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";
import type { Market } from "@/types";

export default function MarketsPage() {
  const router = useRouter();
  const { isAuthenticated } = useAppStore();
  const [markets, setMarkets] = useState<Market[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [filterOutcome, setFilterOutcome] = useState<"all" | "YES" | "NO">("all");

  const loadMarkets = useCallback(async () => {
    setLoading(true);
    try {
      const data = await apiFetch<Market[]>("/markets", { method: "GET" });
      setMarkets(data);
    } catch (_err) {
      // Use mock data if no markets
      setMarkets([
        {
          id: "btc-up-5m",
          question: "Will BTC go UP in the next 5 minutes?",
          outcomes: ["YES", "NO"],
          outcome_prices: [0.52, 0.48],
          volume: 125000,
          active: true,
          expires_at: Date.now() + 300000,
        },
        {
          id: "btc-down-5m",
          question: "Will BTC go DOWN in the next 5 minutes?",
          outcomes: ["YES", "NO"],
          outcome_prices: [0.48, 0.52],
          volume: 98000,
          active: true,
          expires_at: Date.now() + 300000,
        },
        {
          id: "btc-up-1h",
          question: "Will BTC be above $80,000 in 1 hour?",
          outcomes: ["YES", "NO"],
          outcome_prices: [0.65, 0.35],
          volume: 250000,
          active: true,
          expires_at: Date.now() + 3600000,
        },
      ]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // Check both store state and localStorage for auth
    const hasToken = typeof window !== "undefined" && localStorage.getItem("token");
    if (!isAuthenticated && !hasToken) {
      router.push("/login");
      return;
    }
    void loadMarkets();
  }, [isAuthenticated, loadMarkets, router]);

  const filteredMarkets = markets.filter((market) => {
    if (searchQuery && !market.question.toLowerCase().includes(searchQuery.toLowerCase())) {
      return false;
    }
    return true;
  });

  const formatVolume = (volume: number) => {
    if (volume >= 1000000) return `$${(volume / 1000000).toFixed(2)}M`;
    if (volume >= 1000) return `$${(volume / 1000).toFixed(1)}K`;
    return `$${volume}`;
  };

  const formatTimeRemaining = (expiresAt?: number) => {
    if (!expiresAt) return "---";
    const remaining = expiresAt - Date.now();
    if (remaining <= 0) return "Lejárt";
    const minutes = Math.floor(remaining / 60000);
    const seconds = Math.floor((remaining % 60000) / 1000);
    return `${minutes}m ${seconds}s`;
  };

  const selectMarket = (market: Market) => {
    toast.success(`${market.question} kiválasztva`);
    router.push("/");
  };

  return (
    <div
      style={{
        minHeight: "100vh",
        background: "#0b0b0f",
        padding: "2rem",
        position: "relative",
      }}
    >
      {/* Ambient glow */}
      <div
        className="ambient-glow ambient-glow-primary"
        style={{ width: 600, height: 600, top: "10%", left: "20%" }}
      />
      <div
        className="ambient-glow ambient-glow-blue"
        style={{ width: 400, height: 400, bottom: "20%", right: "10%" }}
      />

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        style={{ maxWidth: 1000, margin: "0 auto" }}
      >
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: "1rem", marginBottom: "2rem" }}>
          <div
            style={{
              width: 48,
              height: 48,
              borderRadius: 12,
              background: "rgba(99, 102, 241, 0.15)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <BarChart3 size={24} style={{ color: "#6366f1" }} />
          </div>
          <div>
            <h1 style={{ fontWeight: 700, fontSize: 24, color: "#fafafa" }}>Piacok</h1>
            <span style={{ fontSize: 14, color: "#71717a" }}>Elérhető prediction markets</span>
          </div>
        </div>

        {/* Search and filter */}
        <div style={{ display: "flex", gap: "1rem", marginBottom: "1.5rem" }}>
          <div style={{ position: "relative", flex: 1 }}>
            <Search
              size={20}
              style={{
                color: "#71717a",
                position: "absolute",
                left: 12,
                top: "50%",
                transform: "translateY(-50%)",
              }}
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="input"
              style={{ paddingLeft: 40 }}
              placeholder="Keresés a piacok között..."
            />
          </div>
          <div style={{ display: "flex", gap: "0.5rem" }}>
            {(["all", "YES", "NO"] as const).map((filter) => (
              <button
                key={filter}
                type="button"
                onClick={() => setFilterOutcome(filter)}
                style={{
                  padding: "0.5rem 1rem",
                  borderRadius: 8,
                  fontSize: 14,
                  fontWeight: 500,
                  background:
                    filterOutcome === filter ? "rgba(99, 102, 241, 0.15)" : "rgba(20, 20, 28, 0.6)",
                  color: filterOutcome === filter ? "#6366f1" : "#a1a1aa",
                  border: "none",
                  cursor: "pointer",
                }}
              >
                {filter === "all" ? "Összes" : filter}
              </button>
            ))}
          </div>
        </div>

        {/* Markets grid */}
        {loading ? (
          <div className="glass-card" style={{ padding: "3rem", textAlign: "center" }}>
            <Activity
              size={32}
              style={{ color: "#71717a", marginBottom: "1rem" }}
              className="animate-spin"
            />
            <span style={{ color: "#71717a" }}>Piacok betöltése...</span>
          </div>
        ) : (
          <AnimatePresence>
            {filteredMarkets.length === 0 ? (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="glass-card"
                style={{ padding: "3rem", textAlign: "center" }}
              >
                <BarChart3 size={48} style={{ color: "#71717a", marginBottom: "1rem" }} />
                <h3
                  style={{
                    fontWeight: 600,
                    fontSize: 16,
                    color: "#fafafa",
                    marginBottom: "0.5rem",
                  }}
                >
                  Nincs elérhető piac
                </h3>
                <span style={{ fontSize: 14, color: "#71717a" }}>
                  Jelenleg nincs aktív prediction market
                </span>
              </motion.div>
            ) : (
              <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "1rem" }}>
                {filteredMarkets.map((market) => (
                  <motion.div
                    key={market.id}
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -20 }}
                    whileHover={{ scale: 1.02 }}
                    onClick={() => selectMarket(market)}
                    className="glass-card"
                    style={{ padding: "1.5rem", cursor: "pointer" }}
                  >
                    {/* Market question */}
                    <h3
                      style={{
                        fontWeight: 600,
                        fontSize: 14,
                        color: "#fafafa",
                        marginBottom: "1rem",
                        lineHeight: 1.4,
                      }}
                    >
                      {market.question}
                    </h3>

                    {/* Outcomes */}
                    <div style={{ display: "flex", gap: "1rem", marginBottom: "1rem" }}>
                      {/* YES */}
                      <div
                        style={{
                          flex: 1,
                          padding: "1rem",
                          borderRadius: 8,
                          background: "rgba(34, 197, 94, 0.1)",
                          border: "1px solid rgba(34, 197, 94, 0.2)",
                        }}
                      >
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "0.5rem",
                            marginBottom: "0.5rem",
                          }}
                        >
                          <TrendingUp size={16} style={{ color: "#22c55e" }} />
                          <span style={{ fontWeight: 600, fontSize: 12, color: "#22c55e" }}>
                            YES
                          </span>
                        </div>
                        <span
                          className="price-ticker"
                          style={{ fontSize: 20, fontWeight: 700, color: "#22c55e" }}
                        >
                          {(market.outcome_prices[0] * 100).toFixed(0)}¢
                        </span>
                      </div>

                      {/* NO */}
                      <div
                        style={{
                          flex: 1,
                          padding: "1rem",
                          borderRadius: 8,
                          background: "rgba(239, 68, 68, 0.1)",
                          border: "1px solid rgba(239, 68, 68, 0.2)",
                        }}
                      >
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "0.5rem",
                            marginBottom: "0.5rem",
                          }}
                        >
                          <TrendingDown size={16} style={{ color: "#ef4444" }} />
                          <span style={{ fontWeight: 600, fontSize: 12, color: "#ef4444" }}>
                            NO
                          </span>
                        </div>
                        <span
                          className="price-ticker"
                          style={{ fontSize: 20, fontWeight: 700, color: "#ef4444" }}
                        >
                          {(market.outcome_prices[1] * 100).toFixed(0)}¢
                        </span>
                      </div>
                    </div>

                    {/* Stats */}
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        fontSize: 12,
                      }}
                    >
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <DollarSign size={14} style={{ color: "#71717a" }} />
                        <span style={{ color: "#a1a1aa" }}>{formatVolume(market.volume)}</span>
                      </div>
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <Clock size={14} style={{ color: "#71717a" }} />
                        <span style={{ color: "#a1a1aa" }}>
                          {formatTimeRemaining(market.expires_at)}
                        </span>
                      </div>
                      {market.active && (
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 4,
                            padding: "0.25rem 0.5rem",
                            borderRadius: 4,
                            background: "rgba(34, 197, 94, 0.15)",
                          }}
                        >
                          <div
                            className="status-dot status-dot-active"
                            style={{ width: 6, height: 6 }}
                          />
                          <span style={{ color: "#22c55e" }}>Aktív</span>
                        </div>
                      )}
                    </div>
                  </motion.div>
                ))}
              </div>
            )}
          </AnimatePresence>
        )}
      </motion.div>
    </div>
  );
}
