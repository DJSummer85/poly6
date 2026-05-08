"use client";

import { motion } from "framer-motion";
import { Loader2, Target, TrendingDown, TrendingUp, Zap } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useOrders, useQuickTrade } from "@/hooks/use-api";
import { useAppStore } from "@/store";

type TradeSide = "UP" | "DOWN";

export function QuickTradePanel() {
  const [selectedSide, setSelectedSide] = useState<TradeSide | null>(null);
  const [amount, setAmount] = useState(10);
  const { priceDelta, yesPrice, noPrice, addLog, positions } = useAppStore();
  const quickTrade = useQuickTrade();
  const { data: orders = [] } = useOrders();

  const recentOrders = orders
    .filter((o) => o.status === "FILLED")
    .sort((a, b) => b.filled_at! - a.filled_at!)
    .slice(0, 5);

  const openPositions = positions.filter((p) => p.status === "open");
  const totalExposure = openPositions.reduce((sum, p) => sum + p.stake, 0);
  const totalUnrealizedPnl = openPositions.reduce((sum, p) => sum + (p.pnl ?? 0), 0);

  const handleTrade = async (side: TradeSide) => {
    if (amount <= 0 || amount > 1000) {
      toast.error("Invalid amount", { description: "Amount must be between 1-1000 USDC" });
      return;
    }

    setSelectedSide(side);

    addLog({
      bot_id: 0,
      bot_name: "Manual Trade",
      message: `Placing ${side} bet for ${amount} USDC...`,
      timestamp: Date.now(),
      level: "info",
    });

    quickTrade.mutate(
      { side, amount },
      {
        onSuccess: (result) => {
          if (result.success) {
            toast.success("Bet Placed!", {
              description: `${side} outcome @ ${(side === "UP" ? result.btc_price : result.beat_price)?.toFixed(2)} - Order: ${result.order_id}`,
            });
            addLog({
              bot_id: 0,
              bot_name: "Manual Trade",
              message: `SUCCESS: ${side} bet placed for ${amount} USDC`,
              timestamp: Date.now(),
              level: "success",
            });
            setSelectedSide(null);
          } else {
            toast.error("Trade Failed", { description: result.message });
            setSelectedSide(null);
          }
        },
        onError: (err) => {
          toast.error("Trade Failed", { description: String(err) });
          setSelectedSide(null);
        },
      }
    );
  };

  const isLoading = quickTrade.isPending;
  const isAboveTarget = priceDelta >= 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.1 }}
      className="glass-card flex flex-col gap-4 p-6"
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Zap className="h-5 w-5 text-indigo-500" />
          <span className="text-base font-bold text-zinc-100">Quick Trade</span>
        </div>

        {/* Status Badge - Color-coded */}
        <div
          className={`flex items-center gap-2 rounded-lg px-3 py-2 transition-colors
          ${
            isAboveTarget
              ? "bg-green-500/15 border border-green-500/30"
              : "bg-red-500/15 border border-red-500/30"
          }`}
        >
          <Target className={`h-3.5 w-3.5 ${isAboveTarget ? "text-green-500" : "text-red-500"}`} />
          <span
            className={`text-xs font-semibold ${isAboveTarget ? "text-green-500" : "text-red-500"}`}
          >
            {isAboveTarget ? "Above Target" : "Below Target"}
          </span>
        </div>
      </div>

      {/* Market Odds - Prominent Display */}
      <div className="grid grid-cols-2 gap-4">
        {/* YES/UP */}
        <motion.div
          key={`yes-${yesPrice}`}
          initial={{ scale: 0.95 }}
          animate={{ scale: 1 }}
          className="rounded-xl bg-green-500/8 border border-green-500/20 p-4"
        >
          <div className="flex items-center gap-2 mb-2">
            <TrendingUp className="h-4 w-4 text-green-500" />
            <span className="text-xs font-bold text-green-500">YES (UP)</span>
          </div>
          <div className="text-3xl font-extrabold font-mono text-green-500">
            {(yesPrice * 100).toFixed(1)}¢
          </div>
          <div className="mt-1 text-xs text-zinc-500">
            {yesPrice > 0.5 ? "Market favorite" : ""}
          </div>
        </motion.div>

        {/* NO/DOWN */}
        <motion.div
          key={`no-${noPrice}`}
          initial={{ scale: 0.95 }}
          animate={{ scale: 1 }}
          className="rounded-xl bg-red-500/8 border border-red-500/20 p-4"
        >
          <div className="flex items-center gap-2 mb-2">
            <TrendingDown className="h-4 w-4 text-red-500" />
            <span className="text-xs font-bold text-red-500">NO (DOWN)</span>
          </div>
          <div className="text-3xl font-extrabold font-mono text-red-500">
            {(noPrice * 100).toFixed(1)}¢
          </div>
          <div className="mt-1 text-xs text-zinc-500">{noPrice > 0.5 ? "Market favorite" : ""}</div>
        </motion.div>
      </div>

      {/* Amount Input */}
      <div>
        <div className="mb-2 block text-xs text-zinc-500">Bet Amount (USDC)</div>
        <div className="flex gap-2">
          {[5, 10, 25, 50, 100].map((preset) => (
            <button
              key={preset}
              type="button"
              onClick={() => setAmount(preset)}
              className={`rounded-md px-3 py-2 text-xs font-semibold cursor-pointer transition-all
              ${
                amount === preset
                  ? "bg-indigo-500/15 border border-indigo-500/40 text-indigo-400"
                  : "bg-zinc-900/60 border border-white/10 text-zinc-400 hover:border-white/20"
              }`}
            >
              ${preset}
            </button>
          ))}
          <input
            type="number"
            value={amount}
            onChange={(e) => setAmount(Math.max(1, Math.min(1000, Number(e.target.value))))}
            min={1}
            max={1000}
            className="w-20 rounded-md bg-zinc-900/60 border border-white/10 px-3 py-2 text-xs font-semibold font-mono text-zinc-100 focus:border-indigo-500 focus:outline-none"
          />
        </div>
      </div>

      {/* Trade Buttons */}
      <div className="grid grid-cols-2 gap-4">
        <motion.button
          type="button"
          whileHover={{ scale: isLoading ? 1 : 1.02 }}
          whileTap={{ scale: isLoading ? 1 : 0.98 }}
          onClick={() => handleTrade("UP")}
          disabled={isLoading}
          className={`flex flex-col items-center justify-center gap-1 rounded-xl p-4 font-bold cursor-pointer transition-all
          ${
            selectedSide === "UP"
              ? "bg-green-500/20 border-2 border-green-500 shadow-[0_0_20px_rgba(34,197,94,0.3)]"
              : "bg-green-500/8 border border-green-500/30 hover:border-green-500 hover:shadow-[0_0_20px_rgba(34,197,94,0.25)]"
          }
          ${isLoading && selectedSide !== "UP" ? "opacity-50 cursor-not-allowed" : ""}`}
        >
          <div className="flex items-center gap-2">
            {isLoading && selectedSide === "UP" ? (
              <Loader2 className="h-4 w-4 animate-spin text-green-500" />
            ) : (
              <TrendingUp className="h-4 w-4 text-green-500" />
            )}
            <span className="text-base text-green-500">BET UP</span>
          </div>
          <span className="text-xs text-green-500/70">BTC will exceed target</span>
        </motion.button>

        <motion.button
          type="button"
          whileHover={{ scale: isLoading ? 1 : 1.02 }}
          whileTap={{ scale: isLoading ? 1 : 0.98 }}
          onClick={() => handleTrade("DOWN")}
          disabled={isLoading}
          className={`flex flex-col items-center justify-center gap-1 rounded-xl p-4 font-bold cursor-pointer transition-all
          ${
            selectedSide === "DOWN"
              ? "bg-red-500/20 border-2 border-red-500 shadow-[0_0_20px_rgba(239,68,68,0.3)]"
              : "bg-red-500/8 border border-red-500/30 hover:border-red-500 hover:shadow-[0_0_20px_rgba(239,68,68,0.25)]"
          }
          ${isLoading && selectedSide !== "DOWN" ? "opacity-50 cursor-not-allowed" : ""}`}
        >
          <div className="flex items-center gap-2">
            {isLoading && selectedSide === "DOWN" ? (
              <Loader2 className="h-4 w-4 animate-spin text-red-500" />
            ) : (
              <TrendingDown className="h-4 w-4 text-red-500" />
            )}
            <span className="text-base text-red-500">BET DOWN</span>
          </div>
          <span className="text-xs text-red-500/70">BTC will stay below</span>
        </motion.button>
      </div>

      {/* Open Positions Summary */}
      {openPositions.length > 0 && (
        <div className="rounded-xl bg-zinc-900/60 border border-white/10 p-4">
          <div className="flex items-center justify-between mb-3">
            <span className="text-xs font-bold text-zinc-300">Open Positions</span>
            <div className="flex items-center gap-3 text-[10px] font-mono">
              <span className="text-zinc-500">Exposure</span>
              <span className="text-zinc-100">${totalExposure.toFixed(2)}</span>
              <span className={totalUnrealizedPnl >= 0 ? "text-green-400" : "text-red-400"}>
                {totalUnrealizedPnl >= 0 ? "+" : ""}${totalUnrealizedPnl.toFixed(2)}
              </span>
            </div>
          </div>
          <div className="space-y-1.5">
            {openPositions.map((pos) => (
              <div
                key={pos.id}
                className="flex items-center justify-between rounded-lg bg-zinc-800/50 px-3 py-1.5 text-xs"
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`font-bold ${pos.outcome === "YES" ? "text-green-400" : "text-red-400"}`}
                  >
                    {pos.outcome}
                  </span>
                  <span className="text-zinc-500">@ {(pos.odds * 100).toFixed(0)}¢</span>
                </div>
                <div className="flex items-center gap-3 font-mono">
                  <span className="text-zinc-400">${pos.stake.toFixed(2)}</span>
                  <span className={pos.pnl && pos.pnl >= 0 ? "text-green-400" : "text-red-400"}>
                    {pos.pnl ? `${pos.pnl >= 0 ? "+" : ""}$${pos.pnl.toFixed(2)}` : "—"}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Recent Filled Orders */}
      {recentOrders.length > 0 && (
        <div className="rounded-xl bg-zinc-900/60 border border-white/10 p-4">
          <span className="text-xs font-bold text-zinc-300 mb-3 block">Recent Trades</span>
          <div className="space-y-1.5">
            {recentOrders.map((order) => {
              const isBuy = order.side === "BUY";
              return (
                <div
                  key={order.id}
                  className="flex items-center justify-between rounded-lg bg-zinc-800/50 px-3 py-1.5 text-xs"
                >
                  <div className="flex items-center gap-2">
                    <span className={`font-bold ${isBuy ? "text-green-400" : "text-amber-400"}`}>
                      {isBuy ? "BUY" : "SELL"}
                    </span>
                    <span className="text-zinc-500">{order.outcome}</span>
                    <span className="text-zinc-600">@ {(order.price * 100).toFixed(0)}¢</span>
                  </div>
                  <div className="flex items-center gap-2 font-mono text-zinc-400">
                    <span>${order.size.toFixed(2)}</span>
                    <span className="text-zinc-600 text-[10px]">
                      {order.filled_at
                        ? new Date(order.filled_at).toLocaleTimeString("hu-HU", {
                            hour: "2-digit",
                            minute: "2-digit",
                          })
                        : ""}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </motion.div>
  );
}
