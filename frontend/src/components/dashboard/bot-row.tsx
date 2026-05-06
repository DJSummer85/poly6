"use client";

import { motion } from "framer-motion";
import { Loader2, Play, Square, Trash2 } from "lucide-react";
import { usePortfolio } from "@/hooks";
import { getStrategyColor, strategyAbbr } from "@/lib/utils";
import type { Bot as BotType } from "@/types";

export function BotRow({
  bot,
  isSelected,
  isRunning,
  onToggle,
  onStart,
  onStop,
  onDelete,
  isDeleting,
  isMutating,
}: {
  bot: BotType;
  isSelected: boolean;
  isRunning: boolean;
  onToggle: () => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onDelete: (id: number) => void;
  isDeleting: boolean;
  isMutating: boolean;
}) {
  const color = getStrategyColor(bot.strategy_type);

  return (
    <BotRowInner
      bot={bot}
      color={color}
      isSelected={isSelected}
      isRunning={isRunning}
      onToggle={onToggle}
      onStart={onStart}
      onStop={onStop}
      onDelete={onDelete}
      isDeleting={isDeleting}
      isMutating={isMutating}
    />
  );
}

function BotRowInner({
  bot,
  color,
  isSelected,
  isRunning,
  onToggle,
  onStart,
  onStop,
  onDelete,
  isDeleting,
  isMutating,
}: {
  bot: BotType;
  color: string;
  isSelected: boolean;
  isRunning: boolean;
  onToggle: () => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onDelete: (id: number) => void;
  isDeleting: boolean;
  isMutating: boolean;
}) {
  const { data: portfolio } = usePortfolio(bot.id);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, x: -8 }}
      animate={{ opacity: 1, x: 0 }}
      className={`group flex items-center gap-2.5 rounded-lg border px-3 py-2 transition-all ${
        isRunning
          ? "border-green-500/20 bg-green-500/[0.04]"
          : isSelected
            ? "border-indigo-500/30 bg-indigo-500/10"
            : "border-white/5 bg-white/[0.02] hover:bg-white/[0.04]"
      }`}
    >
      {/* Selection dot */}
      <button
        type="button"
        onClick={isRunning ? undefined : onToggle}
        className={`shrink-0 flex h-4 w-4 items-center justify-center rounded-full border transition-all ${
          isSelected ? "border-indigo-400 bg-indigo-400" : "border-zinc-600 hover:border-zinc-400"
        } ${isRunning ? "opacity-30 cursor-default" : "cursor-pointer"}`}
        title={isSelected ? "Kijelölés törlése" : "Kijelölés"}
      >
        {isSelected && (
          <svg
            width="8"
            height="8"
            viewBox="0 0 8 8"
            fill="none"
            stroke="white"
            strokeWidth="1.5"
            role="img"
            aria-label="Selected"
          >
            <title>Selected</title>
            <path d="M1.5 4l1.5 1.5 3.5-3.5" />
          </svg>
        )}
      </button>

      {/* Status dot */}
      <div className="relative shrink-0">
        {isRunning && (
          <div className="absolute inset-0 rounded-full bg-green-400 animate-ping opacity-40" />
        )}
        <div className={`h-2.5 w-2.5 rounded-full ${isRunning ? "bg-green-400" : "bg-zinc-600"}`} />
      </div>

      {/* Bot info */}
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <span className="truncate text-sm font-medium text-zinc-200">{bot.name}</span>
        <span
          className="shrink-0 rounded px-1.5 py-0.5 text-[8px] font-bold uppercase tracking-wider"
          style={{ background: `${color}20`, color }}
        >
          {strategyAbbr(bot.strategy_type)}
        </span>
      </div>

      {/* Portfolio info */}
      {portfolio && (
        <div className="flex shrink-0 items-center gap-2 text-xs font-mono">
          {/* Balance */}
          <span className="text-zinc-400" title="Egyenleg">
            ${portfolio.balance.toFixed(2)}
          </span>

          {/* Divider */}
          {portfolio.total_trades > 0 && (
            <>
              <span className="text-zinc-600">|</span>
              {/* PnL */}
              <span
                className={`font-semibold ${
                  portfolio.total_pnl >= 0 ? "text-green-400" : "text-red-400"
                }`}
                title="PnL"
              >
                {portfolio.total_pnl >= 0 ? "+" : ""}${portfolio.total_pnl.toFixed(2)}
              </span>
              {/* Win rate */}
              <span className="text-zinc-500" title="Win Rate">
                {portfolio.win_rate.toFixed(0)}%
              </span>
            </>
          )}
        </div>
      )}

      {/* Actions */}
      <div className="flex shrink-0 items-center gap-1" onPointerDown={(e) => e.stopPropagation()}>
        {isRunning ? (
          <button
            type="button"
            onClick={() => onStop(bot.id)}
            disabled={isMutating}
            className="flex h-6 w-6 items-center justify-center rounded-md text-zinc-500 hover:bg-red-500/15 hover:text-red-400 transition-colors cursor-pointer"
            title="Leállítás"
          >
            <Square className="h-3 w-3" />
          </button>
        ) : (
          <button
            type="button"
            onClick={() => onStart(bot.id)}
            disabled={isMutating}
            className="flex h-6 w-6 items-center justify-center rounded-md text-zinc-500 hover:bg-green-500/15 hover:text-green-400 transition-colors cursor-pointer"
            title="Indítás"
          >
            <Play className="h-3 w-3" />
          </button>
        )}
        <button
          type="button"
          onClick={() => onDelete(bot.id)}
          disabled={isDeleting}
          className="flex h-6 w-6 items-center justify-center rounded-md text-zinc-600 opacity-0 group-hover:opacity-100 hover:bg-red-500/15 hover:text-red-400 transition-all cursor-pointer"
          title="Törlés"
        >
          {isDeleting ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Trash2 className="h-3 w-3" />
          )}
        </button>
      </div>
    </motion.div>
  );
}
