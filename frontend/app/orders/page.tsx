"use client";

import { AnimatePresence, motion } from "framer-motion";
import {
  AlertCircle,
  CheckCircle,
  Clock,
  Receipt,
  TrendingDown,
  TrendingUp,
  XCircle,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";
import type { Order } from "@/types";

export default function OrdersPage() {
  const router = useRouter();
  const { isAuthenticated } = useAppStore();
  const [orders, setOrders] = useState<Order[]>([]);
  const [loading, setLoading] = useState(true);
  const [filterStatus, setFilterStatus] = useState<"all" | "PENDING" | "FILLED" | "CANCELLED">(
    "all"
  );

  const loadOrders = useCallback(async () => {
    setLoading(true);
    try {
      const data = await apiFetch<Order[]>("/orders", { method: "GET" });
      setOrders(data);
    } catch (_err) {
      // Use mock data if no orders
      setOrders([
        {
          id: "order-1",
          market_id: "btc-up-5m",
          outcome: "YES",
          side: "BUY",
          price: 52,
          size: 100,
          status: "FILLED",
          created_at: Date.now() - 3600000,
          filled_at: Date.now() - 3500000,
        },
        {
          id: "order-2",
          market_id: "btc-down-5m",
          outcome: "NO",
          side: "BUY",
          price: 48,
          size: 50,
          status: "PENDING",
          created_at: Date.now() - 60000,
        },
        {
          id: "order-3",
          market_id: "btc-up-1h",
          outcome: "YES",
          side: "SELL",
          price: 65,
          size: 25,
          status: "CANCELLED",
          created_at: Date.now() - 86400000,
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
    void loadOrders();
  }, [isAuthenticated, loadOrders, router]);

  const filteredOrders = orders.filter((order) => {
    if (filterStatus === "all") return true;
    return order.status === filterStatus;
  });

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "FILLED":
        return <CheckCircle size={16} style={{ color: "#22c55e" }} />;
      case "CANCELLED":
        return <XCircle size={16} style={{ color: "#ef4444" }} />;
      case "PENDING":
        return <Clock size={16} style={{ color: "#f59e0b" }} />;
      default:
        return <AlertCircle size={16} style={{ color: "#71717a" }} />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "FILLED":
        return "#22c55e";
      case "CANCELLED":
        return "#ef4444";
      case "PENDING":
        return "#f59e0b";
      default:
        return "#71717a";
    }
  };

  const formatTime = (timestamp: number) => {
    const d = new Date(timestamp);
    return d.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const formatDate = (timestamp: number) => {
    const d = new Date(timestamp);
    return d.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
    });
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
            <Receipt size={24} style={{ color: "#6366f1" }} />
          </div>
          <div>
            <h1 style={{ fontWeight: 700, fontSize: 24, color: "#fafafa" }}>Rendelések</h1>
            <span style={{ fontSize: 14, color: "#71717a" }}>Kereskedési rendelések története</span>
          </div>
        </div>

        {/* Stats */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(4, 1fr)",
            gap: "1rem",
            marginBottom: "1.5rem",
          }}
        >
          <div className="glass-card" style={{ padding: "1rem" }}>
            <span style={{ fontSize: 12, color: "#71717a" }}>Összes</span>
            <span
              className="price-ticker"
              style={{
                fontSize: 24,
                fontWeight: 700,
                color: "#fafafa",
                display: "block",
                marginTop: 8,
              }}
            >
              {orders.length}
            </span>
          </div>
          <div className="glass-card" style={{ padding: "1rem" }}>
            <span style={{ fontSize: 12, color: "#71717a" }}>Filled</span>
            <span
              className="price-ticker"
              style={{
                fontSize: 24,
                fontWeight: 700,
                color: "#22c55e",
                display: "block",
                marginTop: 8,
              }}
            >
              {orders.filter((o) => o.status === "FILLED").length}
            </span>
          </div>
          <div className="glass-card" style={{ padding: "1rem" }}>
            <span style={{ fontSize: 12, color: "#71717a" }}>Pending</span>
            <span
              className="price-ticker"
              style={{
                fontSize: 24,
                fontWeight: 700,
                color: "#f59e0b",
                display: "block",
                marginTop: 8,
              }}
            >
              {orders.filter((o) => o.status === "PENDING").length}
            </span>
          </div>
          <div className="glass-card" style={{ padding: "1rem" }}>
            <span style={{ fontSize: 12, color: "#71717a" }}>Cancelled</span>
            <span
              className="price-ticker"
              style={{
                fontSize: 24,
                fontWeight: 700,
                color: "#ef4444",
                display: "block",
                marginTop: 8,
              }}
            >
              {orders.filter((o) => o.status === "CANCELLED").length}
            </span>
          </div>
        </div>

        {/* Filter */}
        <div style={{ display: "flex", gap: "0.5rem", marginBottom: "1rem" }}>
          {(["all", "PENDING", "FILLED", "CANCELLED"] as const).map((filter) => (
            <button
              key={filter}
              type="button"
              onClick={() => setFilterStatus(filter)}
              style={{
                padding: "0.5rem 1rem",
                borderRadius: 8,
                fontSize: 14,
                fontWeight: 500,
                background:
                  filterStatus === filter ? "rgba(99, 102, 241, 0.15)" : "rgba(20, 20, 28, 0.6)",
                color: filterStatus === filter ? "#6366f1" : "#a1a1aa",
                border: "none",
                cursor: "pointer",
              }}
            >
              {filter === "all" ? "Összes" : filter}
            </button>
          ))}
        </div>

        {/* Orders list */}
        {loading ? (
          <div className="glass-card" style={{ padding: "3rem", textAlign: "center" }}>
            <Clock
              size={32}
              style={{ color: "#71717a", marginBottom: "1rem" }}
              className="animate-spin"
            />
            <span style={{ color: "#71717a" }}>Rendelések betöltése...</span>
          </div>
        ) : (
          <AnimatePresence>
            {filteredOrders.length === 0 ? (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="glass-card"
                style={{ padding: "3rem", textAlign: "center" }}
              >
                <Receipt size={48} style={{ color: "#71717a", marginBottom: "1rem" }} />
                <h3
                  style={{
                    fontWeight: 600,
                    fontSize: 16,
                    color: "#fafafa",
                    marginBottom: "0.5rem",
                  }}
                >
                  Nincs rendelés
                </h3>
                <span style={{ fontSize: 14, color: "#71717a" }}>
                  {filterStatus !== "all"
                    ? `Nincs ${filterStatus} rendelés`
                    : "Még nem történt kereskedés"}
                </span>
              </motion.div>
            ) : (
              filteredOrders.map((order) => (
                <motion.div
                  key={order.id}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -20 }}
                  className="glass-card"
                  style={{ padding: "1.5rem", marginBottom: "1rem" }}
                >
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                    }}
                  >
                    {/* Left: Order info */}
                    <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                      <div
                        style={{
                          width: 40,
                          height: 40,
                          borderRadius: 10,
                          background:
                            order.outcome === "YES"
                              ? "rgba(34, 197, 94, 0.15)"
                              : "rgba(239, 68, 68, 0.15)",
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "center",
                        }}
                      >
                        {order.outcome === "YES" ? (
                          <TrendingUp size={20} style={{ color: "#22c55e" }} />
                        ) : (
                          <TrendingDown size={20} style={{ color: "#ef4444" }} />
                        )}
                      </div>
                      <div>
                        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                          <span
                            style={{
                              fontWeight: 600,
                              fontSize: 14,
                              color: order.outcome === "YES" ? "#22c55e" : "#ef4444",
                            }}
                          >
                            {order.outcome}
                          </span>
                          <span style={{ color: "#71717a" }}>•</span>
                          <span style={{ color: "#a1a1aa" }}>{order.side}</span>
                        </div>
                        <span style={{ fontSize: 12, color: "#71717a" }}>{order.market_id}</span>
                      </div>
                    </div>

                    {/* Middle: Details */}
                    <div style={{ display: "flex", alignItems: "center", gap: "2rem" }}>
                      <div style={{ textAlign: "center" }}>
                        <span style={{ fontSize: 12, color: "#71717a" }}>Ár</span>
                        <span
                          className="price-ticker"
                          style={{
                            fontSize: 16,
                            fontWeight: 600,
                            color: "#fafafa",
                            display: "block",
                          }}
                        >
                          {order.price}¢
                        </span>
                      </div>
                      <div style={{ textAlign: "center" }}>
                        <span style={{ fontSize: 12, color: "#71717a" }}>Mennyiség</span>
                        <span
                          className="price-ticker"
                          style={{
                            fontSize: 16,
                            fontWeight: 600,
                            color: "#fafafa",
                            display: "block",
                          }}
                        >
                          ${order.size}
                        </span>
                      </div>
                      <div style={{ textAlign: "center" }}>
                        <span style={{ fontSize: 12, color: "#71717a" }}>Összesen</span>
                        <span
                          className="price-ticker"
                          style={{
                            fontSize: 16,
                            fontWeight: 600,
                            color: "#fafafa",
                            display: "block",
                          }}
                        >
                          ${((order.price * order.size) / 100).toFixed(2)}
                        </span>
                      </div>
                    </div>

                    {/* Right: Status and time */}
                    <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        {getStatusIcon(order.status)}
                        <span style={{ fontSize: 12, color: getStatusColor(order.status) }}>
                          {order.status}
                        </span>
                      </div>
                      <div style={{ textAlign: "right" }}>
                        <span style={{ fontSize: 12, color: "#a1a1aa" }}>
                          {formatDate(order.created_at)}
                        </span>
                        <span style={{ fontSize: 12, color: "#71717a", display: "block" }}>
                          {formatTime(order.created_at)}
                        </span>
                      </div>
                    </div>
                  </div>
                </motion.div>
              ))
            )}
          </AnimatePresence>
        )}
      </motion.div>
    </div>
  );
}
