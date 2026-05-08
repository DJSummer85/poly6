"use client";

import { AnimatePresence, motion } from "framer-motion";
import { Activity, Layers, Terminal, Trash2, TrendingDown, TrendingUp, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useCancelOrder } from "@/hooks/use-api";
import { useAppStore } from "@/store";

type TabType = "positions" | "terminal";

export function ActivityTabs() {
  const [activeTab, setActiveTab] = useState<TabType>("positions");
  const { positions, logs, clearLogs } = useAppStore();
  const cancelOrder = useCancelOrder();

  const formatPnl = (pnl?: number) => {
    if (pnl === undefined) return "---";
    const sign = pnl >= 0 ? "+" : "";
    return `${sign}$${pnl.toFixed(2)}`;
  };

  const formatTime = (timestamp: number) => {
    const d = new Date(timestamp);
    return d.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  const getLevelColor = (level: string) => {
    switch (level) {
      case "success":
        return "text-green-500";
      case "error":
        return "text-red-500";
      case "warn":
        return "text-amber-500";
      default:
        return "text-zinc-400";
    }
  };

  const handleClosePosition = async (positionId: string) => {
    cancelOrder.mutate(positionId, {
      onSuccess: () => toast.success("Position closed"),
      onError: (err) => toast.error("Failed to close position", { description: String(err) }),
    });
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.2 }}
      className="rounded-2xl border border-white/8 bg-white/3 backdrop-blur-xl flex flex-col min-h-64"
    >
      {/* Tab Header */}
      <div className="flex items-center gap-4 py-3 px-4 border-b border-white/8">
        <button
          type="button"
          onClick={() => setActiveTab("positions")}
          className={`flex items-center gap-2 rounded-lg px-3 py-2 font-semibold text-sm cursor-pointer transition-all
          ${
            activeTab === "positions"
              ? "bg-indigo-500/15 border border-indigo-500/30 text-indigo-400"
              : "border border-transparent text-zinc-500 hover:text-zinc-300"
          }`}
        >
          <Layers className="h-4 w-4" />
          <span>Positions</span>
          <span
            className={`rounded px-2 py-0.5 text-xs ${positions.length > 0 ? "bg-indigo-500/20" : "bg-white/10"}`}
          >
            {positions.length}
          </span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab("terminal")}
          className={`flex items-center gap-2 rounded-lg px-3 py-2 font-semibold text-sm cursor-pointer transition-all
          ${
            activeTab === "terminal"
              ? "bg-indigo-500/15 border border-indigo-500/30 text-indigo-400"
              : "border border-transparent text-zinc-500 hover:text-zinc-300"
          }`}
        >
          <Terminal className="h-4 w-4" />
          <span>Terminal</span>
        </button>

        {activeTab === "terminal" && logs.length > 0 && (
          <button
            type="button"
            onClick={clearLogs}
            className="ml-auto flex items-center gap-1 rounded px-2 py-1 text-xs text-zinc-500 hover:text-zinc-300 cursor-pointer transition-colors"
          >
            <Trash2 className="h-3 w-3" />
            Clear
          </button>
        )}
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-auto p-4">
        <AnimatePresence mode="wait" initial={false}>
          {activeTab === "positions" && (
            <motion.div
              key="positions"
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.2 }}
            >
              {positions.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-8 text-zinc-500">
                  <Activity className="h-8 w-8 mb-2 opacity-50" />
                  <div className="text-sm mb-1">No Active Positions</div>
                  <div className="text-xs">Use Quick Trade to place a bet</div>
                </div>
              ) : (
                <div className="flex flex-col gap-3">
                  {positions.map((position) => (
                    <motion.div
                      key={position.id}
                      initial={{ opacity: 0, y: 10 }}
                      animate={{ opacity: 1, y: 0 }}
                      className="rounded-xl bg-zinc-900/60 border border-white/8 p-4"
                    >
                      {/* Position Header */}
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2">
                          {position.outcome === "YES" ? (
                            <TrendingUp className="h-4 w-4 text-green-500" />
                          ) : (
                            <TrendingDown className="h-4 w-4 text-red-500" />
                          )}
                          <span
                            className={`text-sm font-bold ${position.outcome === "YES" ? "text-green-500" : "text-red-500"}`}
                          >
                            {position.outcome === "YES" ? "UP" : "DOWN"}
                          </span>
                          <span className="ml-2 text-xs text-zinc-500">
                            @ {(position.odds * 100).toFixed(0)}¢
                          </span>
                        </div>

                        <button
                          type="button"
                          onClick={() => handleClosePosition(String(position.id))}
                          disabled={cancelOrder.isPending}
                          className="flex h-7 w-7 items-center justify-center rounded-md bg-white/5 text-zinc-500 hover:bg-white/10 hover:text-zinc-300 cursor-pointer transition-colors"
                        >
                          <X className="h-3.5 w-3.5" />
                        </button>
                      </div>

                      {/* Position Details */}
                      <div className="grid grid-cols-3 gap-4 text-xs">
                        <div>
                          <span className="text-zinc-500">Amount</span>
                          <span className="ml-1 font-semibold text-zinc-100">
                            ${position.amount.toFixed(2)}
                          </span>
                        </div>
                        <div>
                          <span className="text-zinc-500">Stake</span>
                          <span className="ml-1 font-semibold text-zinc-100">
                            ${position.stake.toFixed(2)}
                          </span>
                        </div>
                        <div>
                          <span className="text-zinc-500">PnL</span>
                          <span
                            className={`ml-1 font-semibold ${(position.pnl ?? 0) >= 0 ? "text-green-500" : "text-red-500"}`}
                          >
                            {formatPnl(position.pnl)}
                          </span>
                        </div>
                      </div>
                    </motion.div>
                  ))}
                </div>
              )}
            </motion.div>
          )}

          {activeTab === "terminal" && (
            <motion.div
              key="terminal"
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.2 }}
              className="rounded-lg bg-black/60 border border-white/5 p-3 font-mono text-xs min-h-38"
            >
              {logs.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-4 text-zinc-500">
                  <Terminal className="h-6 w-6 mb-2 opacity-50" />
                  <span>No activity yet...</span>
                </div>
              ) : (
                <AnimatePresence initial={false}>
                  {logs.slice(-15).map((log) => (
                    <motion.div
                      key={log.timestamp}
                      initial={{ opacity: 0, x: -10 }}
                      animate={{ opacity: 1, x: 0 }}
                      className="flex items-start gap-2 py-1 border-b border-white/3"
                    >
                      <span className="min-w-18 text-zinc-500">{formatTime(log.timestamp)}</span>
                      <span className={`min-w-20 ${getLevelColor(log.level)}`}>
                        [{log.bot_name}]
                      </span>
                      <span className="text-zinc-100">{log.message}</span>
                    </motion.div>
                  ))}
                </AnimatePresence>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
