"use client";

import { Gamepad2, Zap } from "lucide-react";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";

export function TradingModeToggle() {
  const { tradingMode, setTradingMode } = useAppStore();

  const handleModeChange = async (mode: "demo" | "live") => {
    // Optimistically update UI
    setTradingMode(mode);

    try {
      // Update all bots in the database to match the selected mode
      const trading_mode = mode === "live" ? "live" : "paper";
      await apiFetch("/bots/set-mode", {
        method: "POST",
        body: JSON.stringify({ trading_mode }),
      });
    } catch {
      // Silently fail - the UI still works, just DB not updated
    }
  };

  return (
    <div className="flex items-center gap-1 rounded-full border border-white/10 bg-zinc-950/80 p-1 backdrop-blur-xl shadow-lg shadow-black/20">
      <button
        type="button"
        aria-pressed={tradingMode === "demo"}
        onClick={() => handleModeChange("demo")}
        className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-bold uppercase tracking-wider transition-all ${
          tradingMode === "demo"
            ? "bg-indigo-500 text-white shadow-lg shadow-indigo-950/50"
            : "text-zinc-400 hover:text-zinc-200"
        }`}
      >
        <Gamepad2 className="h-3.5 w-3.5" />
        Demo
      </button>
      <button
        type="button"
        aria-pressed={tradingMode === "live"}
        onClick={() => handleModeChange("live")}
        className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-bold uppercase tracking-wider transition-all ${
          tradingMode === "live"
            ? "bg-emerald-500 text-white shadow-lg shadow-emerald-950/50"
            : "text-zinc-400 hover:text-zinc-200"
        }`}
      >
        <Zap className="h-3.5 w-3.5" />
        Live
      </button>
    </div>
  );
}
