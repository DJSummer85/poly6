"use client";

import { AnimatePresence, motion } from "framer-motion";
import {
  BarChart3,
  CheckCircle2,
  ChevronDown,
  Clock,
  TrendingDown,
  TrendingUp,
  XCircle,
} from "lucide-react";
import { useState } from "react";
import { useAppStore } from "@/store";

function formatBTCPrice(price: number): string {
  if (price >= 1000) {
    return price.toLocaleString("en-US", { minimumFractionDigits: 0, maximumFractionDigits: 0 });
  }
  return price.toFixed(2);
}

function formatTimeDelta(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${minutes}:${String(secs).padStart(2, "0")}`;
}

function timeAgo(timestamp: number): string {
  const diff = Date.now() - timestamp;
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

export function MarketHistory() {
  const { marketHistory } = useAppStore();
  const [isExpanded, setIsExpanded] = useState(true);

  const reversed = [...marketHistory].reverse().slice(0, 5);
  const winRate =
    reversed.length > 0 ? (reversed.filter((r) => r.delta >= 0).length / reversed.length) * 100 : 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.25 }}
      className="rounded-2xl border border-white/8 bg-white/3 backdrop-blur-xl overflow-hidden"
    >
      {/* Header - Clickable to toggle */}
      <button
        type="button"
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex w-full items-center gap-2 border-b border-white/8 px-4 py-3 hover:bg-white/5 transition-colors cursor-pointer"
      >
        <BarChart3 className="h-4 w-4 text-indigo-400 flex-shrink-0" />
        <span className="text-sm font-semibold text-zinc-100">Market Results</span>
        <span className="ml-auto flex items-center gap-2">
          {reversed.length > 0 && (
            <span
              className={`text-xs font-bold ${winRate >= 50 ? "text-green-500" : "text-amber-500"}`}
            >
              {winRate.toFixed(0)}% WR
            </span>
          )}
          <span className="text-xs text-zinc-500">{reversed.length}/5</span>
          <ChevronDown
            className={`h-4 w-4 text-zinc-400 transition-transform ${isExpanded ? "rotate-180" : ""}`}
          />
        </span>
      </button>

      {/* Content */}
      <AnimatePresence initial={false}>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeInOut" }}
            className="overflow-hidden"
          >
            <div className="p-4">
              {reversed.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-6 text-zinc-500">
                  <Clock className="h-8 w-8 mb-2 opacity-50" />
                  <span className="text-sm mb-1">No completed markets yet</span>
                  <span className="text-xs">Results appear when markets close</span>
                </div>
              ) : (
                <div className="flex flex-col gap-2.5">
                  {reversed.map((result, index) => {
                    const exceeded = result.delta >= 0;
                    const absDelta = Math.abs(result.delta);

                    return (
                      <div
                        key={result.endTime}
                        className={`rounded-xl border p-3 transition-colors
                        ${
                          exceeded
                            ? "bg-green-500/5 border-green-500/20 hover:bg-green-500/10"
                            : "bg-red-500/5 border-red-500/20 hover:bg-red-500/10"
                        }`}
                        style={{ transitionDelay: `${index * 30}ms` }}
                      >
                        {/* Result Header */}
                        <div className="flex items-center justify-between mb-2">
                          <div className="flex items-center gap-2">
                            {exceeded ? (
                              <CheckCircle2 className="h-3.5 w-3.5 text-green-500" />
                            ) : (
                              <XCircle className="h-3.5 w-3.5 text-red-500" />
                            )}
                            <span
                              className={`text-xs font-bold ${exceeded ? "text-green-500" : "text-red-500"}`}
                            >
                              {exceeded ? "EXCEEDED" : "BELOW"}
                            </span>
                            <span className="text-[10px] text-zinc-500">
                              #{reversed.length - index}
                            </span>
                          </div>
                          <span className="text-[10px] text-zinc-500">
                            {timeAgo(result.endTime)}
                          </span>
                        </div>

                        {/* Price Details */}
                        <div className="grid grid-cols-4 gap-2 text-xs">
                          <div>
                            <span className="text-zinc-500">Target</span>
                            <p className="font-mono font-semibold text-zinc-200 text-[11px]">
                              ${formatBTCPrice(result.targetPrice)}
                            </p>
                          </div>
                          <div>
                            <span className="text-zinc-500">Final</span>
                            <p className="font-mono font-semibold text-zinc-200 text-[11px]">
                              ${formatBTCPrice(result.finalPrice)}
                            </p>
                          </div>
                          <div>
                            <span className="text-zinc-500">Delta</span>
                            <p
                              className={`font-mono font-bold text-[11px] ${exceeded ? "text-green-500" : "text-red-500"}`}
                            >
                              {exceeded ? "+" : ""}
                              {formatBTCPrice(absDelta)}
                            </p>
                          </div>
                          <div>
                            <span className="text-zinc-500">Move</span>
                            <div className="flex items-center gap-1">
                              {exceeded ? (
                                <TrendingUp className="h-3 w-3 text-green-500" />
                              ) : (
                                <TrendingDown className="h-3 w-3 text-red-500" />
                              )}
                              <p
                                className={`font-mono font-bold text-[11px] ${exceeded ? "text-green-500" : "text-red-500"}`}
                              >
                                {absDelta >= 100 ? "Big" : absDelta >= 50 ? "Medium" : "Small"}
                              </p>
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              {/* Stats Summary */}
              {reversed.length > 0 && (
                <div className="mt-3 rounded-lg bg-zinc-900/50 border border-white/8 p-3">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center gap-4">
                      <div>
                        <span className="text-zinc-500">Win Rate</span>
                        <p
                          className={`font-bold font-mono text-sm ${winRate >= 50 ? "text-green-500" : "text-amber-500"}`}
                        >
                          {winRate.toFixed(0)}%
                        </p>
                      </div>
                      <div>
                        <span className="text-zinc-500">Avg Δ</span>
                        <p className="font-bold font-mono text-sm text-zinc-100">
                          {formatBTCPrice(
                            reversed.reduce((sum, r) => sum + r.delta, 0) / reversed.length
                          )}
                        </p>
                      </div>
                      <div>
                        <span className="text-zinc-500">Max Δ</span>
                        <p className="font-bold font-mono text-sm text-zinc-100">
                          {formatBTCPrice(Math.max(...reversed.map((r) => Math.abs(r.delta))))}
                        </p>
                      </div>
                      <div>
                        <span className="text-zinc-500">Period</span>
                        <p className="font-bold font-mono text-sm text-zinc-100">
                          {formatTimeDelta(reversed[0]?.duration ?? 300)}
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
