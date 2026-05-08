"use client";

import { motion } from "framer-motion";
import { AlertTriangle, Award, BarChart3, TrendingDown, TrendingUp, Trophy } from "lucide-react";
import { useAggregatePortfolio, useBots } from "@/hooks";
import { STRATEGY_LABELS } from "@/types";

interface StrategyStats {
  strategyType: string;
  name: string;
  category: string;
  botCount: number;
  totalTrades: number;
  totalPnl: number;
  winningTrades: number;
  losingTrades: number;
  winRate: number;
  avgPnlPerTrade: number;
}

export function StrategyPerformance() {
  const { data: agg, isLoading: aggLoading } = useAggregatePortfolio();
  const { data: bots = [], isLoading: botsLoading } = useBots();

  if (aggLoading || botsLoading) {
    return (
      <div className="flex flex-col gap-3 p-6">
        <div className="h-4 w-32 rounded bg-zinc-800 animate-pulse" />
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="h-24 rounded-xl bg-zinc-800 animate-pulse" />
          ))}
        </div>
      </div>
    );
  }

  // Use aggregate portfolio data as single source of truth — bot configs only for strategy metadata
  const strategyMap = new Map<string, StrategyStats>();

  if (agg?.bots) {
    for (const botPortfolio of agg.bots) {
      const bot = bots.find((b) => b.id === botPortfolio.bot_id);
      if (!bot) continue;

      const key = bot.strategy_type;
      const config = STRATEGY_LABELS[bot.strategy_type as keyof typeof STRATEGY_LABELS];
      const realWins =
        botPortfolio.winning_trades ??
        Math.round((botPortfolio.win_rate / 100) * botPortfolio.total_trades);
      const existing = strategyMap.get(key);

      if (existing) {
        existing.botCount++;
        existing.totalTrades += botPortfolio.total_trades;
        existing.totalPnl += botPortfolio.total_pnl;
        existing.winningTrades += realWins;
      } else {
        strategyMap.set(key, {
          strategyType: key,
          name: config?.name ?? key,
          category: config?.category ?? "Other",
          botCount: 1,
          totalTrades: botPortfolio.total_trades,
          totalPnl: botPortfolio.total_pnl,
          winningTrades: realWins,
          losingTrades: botPortfolio.losing_trades ?? 0,
          winRate: 0,
          avgPnlPerTrade: 0,
        });
      }
    }
  }

  // Bots without portfolio yet — show with zero trades
  for (const bot of bots) {
    if (!agg?.bots?.some((bp) => bp.bot_id === bot.id)) {
      const key = bot.strategy_type;
      const config = STRATEGY_LABELS[bot.strategy_type as keyof typeof STRATEGY_LABELS];
      const existing = strategyMap.get(key);

      if (existing) {
        existing.botCount++;
      } else {
        strategyMap.set(key, {
          strategyType: key,
          name: config?.name ?? key,
          category: config?.category ?? "Other",
          botCount: 1,
          totalTrades: 0,
          totalPnl: 0,
          winningTrades: 0,
          losingTrades: 0,
          winRate: 0,
          avgPnlPerTrade: 0,
        });
      }
    }
  }

  const strategies = Array.from(strategyMap.values()).sort((a, b) => b.totalPnl - a.totalPnl);

  // Recalculate derived metrics from aggregated totals
  for (const s of strategies) {
    s.winRate = s.totalTrades > 0 ? (s.winningTrades / s.totalTrades) * 100 : 0;
    s.losingTrades = s.totalTrades - s.winningTrades;
    s.avgPnlPerTrade = s.totalTrades > 0 ? s.totalPnl / s.totalTrades : 0;
  }

  if (strategies.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 p-8 text-center">
        <BarChart3 className="h-8 w-8 text-zinc-700" />
        <span className="text-sm text-zinc-500">Még nincsenek stratégiák</span>
        <span className="text-xs text-zinc-600">
          Hozz létre legalább egy botot a stratégiai teljesítmény megtekintéséhez
        </span>
      </div>
    );
  }

  const best = strategies[0];
  const worst = strategies[strategies.length - 1];
  const totalTrades = strategies.reduce((sum, s) => sum + s.totalTrades, 0);

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Summary stats */}
      <div className="grid grid-cols-3 gap-3">
        <div className="rounded-lg bg-emerald-500/5 border border-emerald-500/20 px-3 py-2">
          <div className="text-[10px] uppercase tracking-wider text-zinc-500">Stratégiák</div>
          <div className="text-lg font-extrabold font-mono text-emerald-400">
            {strategies.length}
          </div>
        </div>
        <div className="rounded-lg bg-violet-500/5 border border-violet-500/20 px-3 py-2">
          <div className="flex items-center gap-1">
            <Award className="h-3 w-3 text-violet-400" />
            <span className="text-[10px] uppercase tracking-wider text-zinc-500">Legjobb</span>
          </div>
          <div className="text-sm font-bold font-mono text-violet-400">{best.name}</div>
        </div>
        {totalTrades > 0 && (
          <div className="rounded-lg bg-blue-500/5 border border-blue-500/20 px-3 py-2">
            <div className="text-[10px] uppercase tracking-wider text-zinc-500">Összes trade</div>
            <div className="text-lg font-extrabold font-mono text-blue-400">{totalTrades}</div>
          </div>
        )}
      </div>

      {/* Strategy cards */}
      <div className="space-y-2">
        {strategies.map((strategy) => {
          const isPositive = strategy.totalPnl >= 0;
          const wr = strategy.totalTrades > 0 ? strategy.winRate : 0;

          return (
            <motion.div
              key={strategy.strategyType}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              className="rounded-xl border border-white/8 bg-white/[0.03] p-3 hover:bg-white/[0.05] transition-colors"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div
                    className={`flex h-8 w-8 items-center justify-center rounded-lg ${
                      isPositive ? "bg-green-500/15" : "bg-red-500/15"
                    }`}
                  >
                    {isPositive ? (
                      <TrendingUp className="h-4 w-4 text-green-400" />
                    ) : (
                      <TrendingDown className="h-4 w-4 text-red-400" />
                    )}
                  </div>
                  <div>
                    <div className="text-sm font-bold text-zinc-100">{strategy.name}</div>
                    <div className="text-[10px] text-zinc-500">
                      {strategy.category} · {strategy.botCount} bot
                      {strategy.botCount > 1 ? "s" : ""}
                    </div>
                  </div>
                </div>

                <div className="text-right">
                  <div
                    className={`text-base font-extrabold font-mono ${
                      isPositive ? "text-green-400" : "text-red-400"
                    }`}
                  >
                    {isPositive ? "+" : ""}${strategy.totalPnl.toFixed(2)}
                  </div>
                  <div className="text-[10px] text-zinc-500">
                    {strategy.totalTrades} trades · {wr.toFixed(1)}% WR
                  </div>
                </div>
              </div>

              {/* Win rate bar */}
              {strategy.totalTrades > 0 && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="flex-1 flex h-1 rounded-full bg-zinc-800 overflow-hidden">
                    <div
                      className="bg-green-500/50 transition-all duration-500"
                      style={{ width: `${wr}%` }}
                    />
                    <div
                      className="bg-red-500/30 transition-all duration-500"
                      style={{ width: `${100 - wr}%` }}
                    />
                  </div>
                  <span className="text-[10px] font-mono text-zinc-500">
                    ${strategy.avgPnlPerTrade >= 0 ? "+" : ""}${strategy.avgPnlPerTrade.toFixed(2)}
                    /trade
                  </span>
                </div>
              )}

              {strategy.totalTrades === 0 && (
                <div className="mt-2 flex items-center gap-1 text-[10px] text-zinc-600">
                  <AlertTriangle className="h-3 w-3" />
                  Még nem kereskedett
                </div>
              )}
            </motion.div>
          );
        })}
      </div>

      {/* Footer: best vs worst comparison */}
      {strategies.length >= 2 && (
        <div className="rounded-lg bg-zinc-900/60 border border-white/10 px-3 py-2">
          <div className="flex items-center justify-between text-[10px]">
            <div className="flex items-center gap-1">
              <Trophy className="h-3 w-3 text-amber-400" />
              <span className="text-zinc-500">Best vs Worst:</span>
              <span className="text-green-400 font-mono">+${best.totalPnl.toFixed(2)}</span>
              <span className="text-zinc-600">vs</span>
              <span className="text-red-400 font-mono">
                {worst.totalPnl >= 0 ? "+" : ""}${worst.totalPnl.toFixed(2)}
              </span>
            </div>
            <span className="text-zinc-600 font-mono">
              Spread: ${(best.totalPnl - worst.totalPnl).toFixed(2)}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
