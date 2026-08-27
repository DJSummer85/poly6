"use client";

import { motion } from "framer-motion";
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  BarChart3,
  Bot as BotIcon,
  CheckCircle,
  Clock,
  Crosshair,
  Flame,
  TrendingDown,
  TrendingUp,
  Zap,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useMarketCondition, useMarketRecommendation } from "@/hooks";

const regimeConfig = {
  trending: {
    icon: TrendingUp,
    color: "text-emerald-400",
    bg: "bg-emerald-500/10",
    border: "border-emerald-500/20",
    label: "TREND",
    emoji: "📈",
  },
  ranging: {
    icon: Activity,
    color: "text-blue-400",
    bg: "bg-blue-500/10",
    border: "border-blue-500/20",
    label: "RANGING",
    emoji: "📊",
  },
  volatile: {
    icon: Flame,
    color: "text-orange-400",
    bg: "bg-orange-500/10",
    border: "border-orange-500/20",
    label: "VOLATILE",
    emoji: "⚡",
  },
  unknown: {
    icon: AlertTriangle,
    color: "text-zinc-400",
    bg: "bg-zinc-500/10",
    border: "border-zinc-500/20",
    label: "ISMERETLEN",
    emoji: "❓",
  },
};

function SuitabilityBar({ score }: { score: number }) {
  const color =
    score >= 0.8
      ? "bg-emerald-400"
      : score >= 0.6
        ? "bg-yellow-400"
        : score >= 0.4
          ? "bg-orange-400"
          : "bg-red-400";

  return (
    <div className="w-full bg-zinc-800 rounded-full h-1.5">
      <motion.div
        className={`h-1.5 rounded-full ${color}`}
        initial={{ width: 0 }}
        animate={{ width: `${score * 100}%` }}
        transition={{ duration: 0.5, ease: "easeOut" }}
      />
    </div>
  );
}

export function MarketConditionPanel() {
  const { data: conditionData, isLoading: conditionLoading } = useMarketCondition();
  const { data: recommendData, isLoading: recommendLoading } = useMarketRecommendation();
  const [lastUpdate, setLastUpdate] = useState<Date>(new Date());

  useEffect(() => {
    if (conditionData) setLastUpdate(new Date());
  }, [conditionData]);

  if (conditionLoading && !conditionData) {
    return (
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 backdrop-blur-xl p-4">
        <div className="flex items-center gap-2 mb-3">
          <Crosshair className="h-4 w-4 text-zinc-500" />
          <span className="text-xs font-bold uppercase tracking-wider text-zinc-500">
            Piaci Allapot
          </span>
        </div>
        <div className="flex items-center justify-center py-6">
          <div className="animate-spin rounded-full h-6 w-6 border-2 border-zinc-700 border-t-emerald-400" />
        </div>
      </div>
    );
  }

  const condition = conditionData?.condition;
  const regime = condition?.regime || "unknown";
  const config = regimeConfig[regime as keyof typeof regimeConfig] || regimeConfig.unknown;
  const RegimeIcon = config.icon;

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className={`rounded-xl border ${config.border} ${config.bg} backdrop-blur-xl p-4`}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <Crosshair className="h-4 w-4 text-zinc-500" />
          <span className="text-xs font-bold uppercase tracking-wider text-zinc-500">
            Piaci Allapot
          </span>
        </div>
        <div className="flex items-center gap-1 text-[10px] text-zinc-600">
          <Clock className="h-3 w-3" />
          {lastUpdate.toLocaleTimeString("hu-HU")}
        </div>
      </div>

      {/* Regime Badge */}
      <div className="flex items-center gap-3 mb-3">
        <div className={`flex items-center gap-2 rounded-lg px-3 py-1.5 ${config.bg} border ${config.border}`}>
          <RegimeIcon className={`h-4 w-4 ${config.color}`} />
          <span className={`text-sm font-extrabold ${config.color}`}>
            {config.emoji} {config.label}
          </span>
        </div>
        {condition && (
          <div className="text-xs text-zinc-500">
           Bizalom: {Math.round(condition.confidence * 100)}%
          </div>
        )}
      </div>

      {/* Summary */}
      {condition && (
        <p className="text-xs text-zinc-400 mb-3 leading-relaxed">
          {condition.summary}
        </p>
      )}

      {/* Metrics */}
      {condition && (
        <div className="grid grid-cols-3 gap-2 mb-3">
          <div className="rounded-lg bg-zinc-800/50 p-2">
            <div className="text-[10px] text-zinc-600 uppercase">Valtozas</div>
            <div className={`text-sm font-bold font-mono ${
              condition.avg_price_change > 0 ? "text-emerald-400" : condition.avg_price_change < 0 ? "text-red-400" : "text-zinc-400"
            }`}>
              {condition.avg_price_change > 0 ? "+" : ""}
              {condition.avg_price_change.toFixed(3)}%
            </div>
          </div>
          <div className="rounded-lg bg-zinc-800/50 p-2">
            <div className="text-[10px] text-zinc-600 uppercase">Volatilitas</div>
            <div className="text-sm font-bold font-mono text-zinc-300">
              {Math.round(condition.volatility * 100)}%
            </div>
          </div>
          <div className="rounded-lg bg-zinc-800/50 p-2">
            <div className="text-[10px] text-zinc-600 uppercase">Tartomany</div>
            <div className="text-sm font-bold font-mono text-zinc-300">
              {condition.price_range_pct.toFixed(2)}%
            </div>
          </div>
        </div>
      )}

      {/* Recommended Strategies */}
      {condition && condition.recommended_strategies.length > 0 && (
        <div>
          <div className="text-[10px] font-bold uppercase tracking-wider text-zinc-600 mb-2">
            Ajanlott Strategiak
          </div>
          <div className="space-y-2">
            {condition.recommended_strategies.slice(0, 3).map((rec) => (
              <div
                key={rec.strategy}
                className="flex items-center gap-2 rounded-lg bg-zinc-800/30 px-2.5 py-1.5"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-bold text-zinc-300">{rec.name}</span>
                    <span className="text-[10px] text-zinc-600">
                      {Math.round(rec.suitability * 100)}%
                    </span>
                  </div>
                  <div className="text-[10px] text-zinc-500 truncate">{rec.reason}</div>
                </div>
                <SuitabilityBar score={rec.suitability} />
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Bot Recommendations */}
      {recommendData && recommendData.recommendations.length > 0 && (
        <div className="mt-3 pt-3 border-t border-zinc-800">
          <div className="text-[10px] font-bold uppercase tracking-wider text-zinc-600 mb-2">
            Legjobb Botok Most
          </div>
          <div className="space-y-1.5">
            {recommendData.recommendations.slice(0, 3).map((rec, i) => (
              <div
                key={rec.bot_id}
                className="flex items-center gap-2 rounded-lg bg-zinc-800/30 px-2.5 py-1.5"
              >
                <div className={`w-5 h-5 rounded flex items-center justify-center text-[10px] font-bold ${
                  i === 0 ? "bg-emerald-500/20 text-emerald-400" : "bg-zinc-700 text-zinc-500"
                }`}>
                  {i + 1}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5">
                    <BotIcon className="h-3 w-3 text-zinc-500" />
                    <span className="text-xs font-bold text-zinc-300 truncate">{rec.bot_name}</span>
                  </div>
                  <div className="text-[10px] text-zinc-500 truncate">{rec.reason}</div>
                </div>
                <div className={`text-xs font-bold ${
                  rec.match_score >= 0.8 ? "text-emerald-400" : rec.match_score >= 0.6 ? "text-yellow-400" : "text-zinc-500"
                }`}>
                  {Math.round(rec.match_score * 100)}%
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </motion.div>
  );
}
