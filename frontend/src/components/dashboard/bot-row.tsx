"use client";

import { motion } from "framer-motion";
import { Loader2, Play, Square, Trash2 } from "lucide-react";
import { usePortfolio } from "@/hooks";
import { getStrategyColor, strategyAbbr } from "@/lib/utils";
import { useAppStore } from "@/store";
import type { Bot as BotType } from "@/types";

function RiskBadge({ riskMultiplier, adjustedConfidence }: { riskMultiplier: number; adjustedConfidence?: number }) {
  const riskColor = riskMultiplier >= 0.8 ? "text-green-400" : riskMultiplier >= 0.5 ? "text-amber-400" : "text-red-400";
  return (
    <div className="flex items-center gap-2 shrink-0">
      <div className="flex flex-col items-end leading-none">
        <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter leading-none mb-0.5">Risk</span>
        <span className={`text-[10px] font-bold ${riskColor}`}>×{riskMultiplier.toFixed(2)}</span>
      </div>
      {adjustedConfidence !== undefined && (
        <div className="flex flex-col items-end leading-none">
          <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter leading-none mb-0.5">Adj.Conf</span>
          <span className="text-[10px] font-bold text-indigo-400">{(adjustedConfidence * 100).toFixed(0)}%</span>
        </div>
      )}
    </div>
  );
}

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
  const activities = useAppStore(state => state.botActivities[bot.id]) || [];

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
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <span className="truncate text-sm font-bold text-zinc-100 tracking-tight min-w-[120px]">{bot.name}</span>
        
        <div className="flex items-center gap-2 shrink-0">
          <span
            className="rounded px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider shadow-sm"
            style={{ background: `${color}25`, color, border: `1px solid ${color}40` }}
          >
            {strategyAbbr(bot.strategy_type)}
          </span>
          <span 
            className={`rounded px-1.5 py-0.5 text-[9px] font-black uppercase tracking-widest shadow-sm border ${
              bot.asset === 'BTC' ? 'bg-orange-500/10 text-orange-400 border-orange-500/30' :
              bot.asset === 'ETH' ? 'bg-blue-500/10 text-blue-400 border-blue-500/30' :
              bot.asset === 'SOL' ? 'bg-purple-500/10 text-purple-400 border-purple-500/30' :
              bot.asset === 'XRP' ? 'bg-teal-500/10 text-teal-400 border-teal-500/30' :
              'bg-zinc-500/10 text-zinc-400 border-zinc-500/30'
            }`}
          >
            {bot.asset}
          </span>
        </div>

        {/* Live Status / Last Action */}
        <div className="flex items-center gap-4 flex-1 min-w-0 px-4">
          {(() => {
            const lastDecision = [...activities].reverse().find(a => a.type === "trade_decision");
            
            if (!lastDecision) return <span className="text-[10px] text-zinc-600 italic">Várakozás jelzésre...</span>;
            
            const data = lastDecision.data as any;
            const isHold = data.outcome === "HOLD";
            
            return (
              <div className="flex items-center gap-4 w-full animate-in fade-in slide-in-from-left-1 duration-500">
                {!isHold && (
                  <div className="flex items-center gap-2 shrink-0">
                    <span className={`px-1.5 py-0.5 rounded text-[10px] font-black ${
                      data.outcome === "YES" ? "bg-green-500 text-black" : "bg-red-500 text-white"
                    }`}>
                      {data.outcome === "YES" ? "UP" : "DOWN"}
                    </span>
                    <span className="text-xs font-bold text-zinc-300">${data.betSize?.toFixed(1) || bot.bet_size.toFixed(1)}</span>
                  </div>
                )}
                
                <div className="flex flex-col min-w-0 flex-1">
                  <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter leading-none mb-1">Last Thought</span>
                  <span className="text-[11px] text-zinc-400 truncate leading-none italic">
                    "{data.reason || "Elemzés..."}"
                  </span>
                </div>

                {data.confidence > 0 && (
                  <div className="flex flex-col items-end shrink-0">
                    <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter leading-none mb-1">Confidence</span>
                    <div className="flex items-center gap-1.5">
                      <div className="w-12 h-1 bg-zinc-800 rounded-full overflow-hidden">
                        <div 
                          className="h-full bg-indigo-500 transition-all duration-1000" 
                          style={{ width: `${data.confidence * 100}%` }}
                        />
                      </div>
                      <span className="text-[10px] font-bold text-indigo-400">{(data.confidence * 100).toFixed(0)}%</span>
                    </div>
                  </div>
                )}

                {/* Risk Metrics */}
                {data.riskMultiplier !== undefined && (
                  <RiskBadge
                    riskMultiplier={data.riskMultiplier as number}
                    adjustedConfidence={data.adjustedConfidence as number | undefined}
                  />
                )}
              </div>
            );
          })()}
        </div>
      </div>

      {/* Portfolio info */}
      {portfolio && (
        <div className="flex shrink-0 items-center gap-3 text-[11px] font-mono bg-black/20 px-3 py-1.5 rounded-lg border border-white/5">
          {/* Balance */}
          <div className="flex flex-col items-end leading-none">
            <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter mb-0.5">Balance</span>
            <span className="text-zinc-100 font-bold">${portfolio.balance.toFixed(2)}</span>
          </div>

          <div className="h-5 w-px bg-white/10" />

          {/* ROI */}
          <div className="flex flex-col items-end leading-none">
            <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter mb-0.5">ROI</span>
            <span className={`font-bold ${portfolio.total_pnl >= 0 ? "text-emerald-400" : "text-rose-400"}`}>
              {((portfolio.balance - 100) / 100 * 100).toFixed(1)}%
            </span>
          </div>

          <div className="h-5 w-px bg-white/10" />

          {/* PnL and Stats */}
          {portfolio.total_trades > 0 ? (
            <div className="flex items-center gap-4">
              <div className="flex flex-col items-end leading-none">
                <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter mb-0.5">PnL</span>
                <span className={`font-bold ${portfolio.total_pnl >= 0 ? "text-green-400" : "text-red-400"}`}>
                  {portfolio.total_pnl >= 0 ? "+" : ""}${portfolio.total_pnl.toFixed(2)}
                </span>
              </div>
              <div className="flex flex-col items-end leading-none">
                <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-tighter mb-0.5">WR</span>
                <span className="text-zinc-400 font-bold">{portfolio.win_rate.toFixed(0)}%</span>
              </div>
            </div>
          ) : (
            <span className="text-[9px] text-zinc-600 uppercase font-bold italic px-2">No active trades</span>
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
