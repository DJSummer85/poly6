"use client";

import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, ChevronUp, LineChart } from "lucide-react";
import { TradingViewWidget } from "@/components/ui/trading-view-widget";

interface ChartPanelProps {
  expanded: boolean;
  onToggle: () => void;
}

export function ChartPanel({ expanded, onToggle }: ChartPanelProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.15 }}
      className="rounded-2xl border border-white/8 bg-white/3 backdrop-blur-xl flex flex-col p-4"
    >
      {/* Header with Toggle */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <LineChart className="h-4 w-4 text-indigo-500" />
          <span className="text-sm font-semibold text-zinc-100">BTC Chart</span>
        </div>

        <button
          type="button"
          onClick={onToggle}
          className={`flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-semibold cursor-pointer transition-all
          ${
            expanded
              ? "bg-indigo-500/15 border border-indigo-500/30 text-indigo-400"
              : "bg-zinc-900/60 border border-white/10 text-zinc-500 hover:border-white/20"
          }`}
        >
          {expanded ? (
            <>
              <ChevronUp className="h-4 w-4" />
              <span>Collapse</span>
            </>
          ) : (
            <>
              <ChevronDown className="h-4 w-4" />
              <span>Expand</span>
            </>
          )}
        </button>
      </div>

      {/* Chart Container */}
      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 400, opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3, ease: "easeInOut" }}
            className="rounded-xl bg-zinc-950 border border-white/8 overflow-hidden"
          >
            <TradingViewWidget symbol="BINANCE:BTCUSDT" interval="5" height={400} />
          </motion.div>
        )}
      </AnimatePresence>

      {/* Collapsed State */}
      {!expanded && (
        <div className="flex items-center justify-center py-2 text-xs text-zinc-500">
          <span>Click "Expand" to view BTC/USDT chart</span>
        </div>
      )}
    </motion.div>
  );
}
