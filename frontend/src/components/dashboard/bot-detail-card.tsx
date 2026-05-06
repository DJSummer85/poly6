"use client";

import { motion } from "framer-motion";
import {
  Bot,
  DollarSign,
  Loader2,
  Play,
  Square,
  TrendingDown,
  TrendingUp,
  Zap,
} from "lucide-react";
import { usePortfolio } from "@/hooks";
import { getStrategyColor } from "@/lib/utils";
import type { Bot as BotType } from "@/types";
import { LiveBotActivityCard } from "./live-bot-activity-card";

// ── Detailed Bot Card ──

export function BotDetailCard({
  bot,
  isRunning,
  onStart,
  onStop,
  isMutating,
}: {
  bot: BotType;
  isRunning: boolean;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  isMutating: boolean;
}) {
  const { data: portfolio } = usePortfolio(bot.id);
  const color = getStrategyColor(bot.strategy_type);

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -8 }}
      className={`rounded-lg border overflow-hidden ${
        isRunning ? "border-green-500/20 bg-green-500/[0.04]" : "border-white/5 bg-white/[0.03]"
      }`}
    >
      {isRunning && (
        <div className="flex items-center gap-2 px-3 py-1.5 border-b border-green-500/10 bg-green-500/5">
          <div className="relative">
            <div className="absolute inset-0 rounded-full bg-green-400 animate-ping opacity-30" />
            <div className="relative h-2 w-2 rounded-full bg-green-400" />
          </div>
          <span className="text-[10px] font-semibold text-green-400 uppercase tracking-wider">
            Trading Active
          </span>
          <span className="text-[10px] text-zinc-500 ml-auto">
            {bot.trading_mode === "live" ? "Live" : "Paper"}
          </span>
        </div>
      )}

      <div className="p-3">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2.5">
            <div
              className="flex h-8 w-8 items-center justify-center rounded-lg"
              style={{ background: `${color}15` }}
            >
              {isRunning ? (
                <Loader2 className="h-4 w-4 animate-spin" style={{ color }} />
              ) : (
                <Bot className="h-4 w-4" style={{ color: `${color}80` }} />
              )}
            </div>
            <div>
              <div className="text-sm font-semibold text-zinc-100">{bot.name}</div>
              <div className="flex items-center gap-1.5 mt-0.5">
                <span
                  className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase tracking-wider"
                  style={{ background: `${color}20`, color }}
                >
                  {bot.strategy_type}
                </span>
                {bot.use_kelly && (
                  <span className="rounded px-1.5 py-0.5 text-[8px] font-bold uppercase tracking-wider bg-amber-500/15 text-amber-400">
                    Kelly
                  </span>
                )}
              </div>
            </div>
          </div>

          <div className="flex gap-1">
            {isRunning ? (
              <button
                type="button"
                onClick={() => onStop(bot.id)}
                disabled={isMutating}
                className="flex h-7 w-7 items-center justify-center rounded-md bg-red-500/10 text-red-400 hover:bg-red-500/20 transition-colors cursor-pointer"
                title="Stop"
              >
                <Square className="h-3.5 w-3.5" />
              </button>
            ) : (
              <button
                type="button"
                onClick={() => onStart(bot.id)}
                disabled={isMutating}
                className="flex h-7 w-7 items-center justify-center rounded-md bg-green-500/10 text-green-400 hover:bg-green-500/20 transition-colors cursor-pointer"
                title="Start"
              >
                <Play className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2 mb-3 flex-wrap">
          <div className="flex items-center gap-1 rounded-md bg-white/5 px-2 py-1">
            <DollarSign className="h-3 w-3 text-zinc-500" />
            <span className="text-[10px] font-mono text-zinc-300">${bot.bet_size.toFixed(2)}</span>
          </div>
          {bot.use_kelly && (
            <div className="flex items-center gap-1 rounded-md bg-white/5 px-2 py-1">
              <Zap className="h-3 w-3 text-amber-400" />
              <span className="text-[10px] font-mono text-zinc-300">
                {(bot.kelly_fraction * 100).toFixed(0)}%
              </span>
            </div>
          )}
          <div className="flex items-center gap-1 rounded-md bg-white/5 px-2 py-1">
            <TrendingDown className="h-3 w-3 text-red-400" />
            <span className="text-[10px] font-mono text-zinc-300">
              SL {(bot.stop_loss * 100).toFixed(0)}%
            </span>
          </div>
          <div className="flex items-center gap-1 rounded-md bg-white/5 px-2 py-1">
            <TrendingUp className="h-3 w-3 text-green-400" />
            <span className="text-[10px] font-mono text-zinc-300">
              TP {(bot.take_profit * 100).toFixed(0)}%
            </span>
          </div>
        </div>

        {portfolio && portfolio.balance != null && portfolio.total_trades > 0 && (
          <div className="grid grid-cols-3 gap-2">
            <div className="rounded-lg bg-green-500/5 border border-green-500/10 px-2.5 py-2">
              <span className="text-[9px] uppercase text-zinc-500 font-semibold">PnL</span>
              <div
                className={`text-sm font-extrabold font-mono ${
                  portfolio.total_pnl >= 0 ? "text-green-400" : "text-red-400"
                }`}
              >
                {portfolio.total_pnl >= 0 ? "+" : ""}${portfolio.total_pnl.toFixed(2)}
              </div>
            </div>
            <div className="rounded-lg bg-violet-500/5 border border-violet-500/10 px-2.5 py-2">
              <span className="text-[9px] uppercase text-zinc-500 font-semibold">Win Rate</span>
              <div className="text-sm font-extrabold font-mono text-violet-400">
                {portfolio.win_rate.toFixed(1)}%
              </div>
            </div>
            <div className="rounded-lg bg-blue-500/5 border border-blue-500/10 px-2.5 py-2">
              <span className="text-[9px] uppercase text-zinc-500 font-semibold">Trades</span>
              <div className="text-sm font-extrabold font-mono text-blue-400">
                {portfolio.total_trades}
              </div>
              <div className="flex gap-1 mt-0.5">
                <span className="text-[9px] text-green-400">{portfolio.winning_trades}W</span>
                <span className="text-[9px] text-red-400">{portfolio.losing_trades}L</span>
              </div>
            </div>
          </div>
        )}

        {(!portfolio || portfolio.balance == null || portfolio.total_trades === 0) && (
          <div className="rounded-lg bg-white/3 border border-white/5 px-3 py-2 text-center">
            <span className="text-[10px] text-zinc-500">Még nincsenek trading adatok</span>
          </div>
        )}

        {/* Live activity feed for running bots */}
        {isRunning && (
          <div className="mt-3 pt-3 border-t border-white/5">
            <LiveBotActivityCard botId={bot.id} />
          </div>
        )}
      </div>
    </motion.div>
  );
}
