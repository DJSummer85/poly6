"use client";

import { motion } from "framer-motion";
import {
  AlertTriangle,
  Bot,
  ChevronDown,
  ChevronUp,
  Shield,
  ShieldAlert,
  ShieldCheck,
  TrendingDown,
} from "lucide-react";
import { useState } from "react";
import { useAppStore } from "@/store";

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString("hu-HU", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function RiskMetricsPanel() {
  const botRiskMetrics = useAppStore((s) => s.botRiskMetrics);
  const bots = useAppStore((s) => s.bots);
  const [expanded, setExpanded] = useState(false);

  const metricEntries = Object.entries(botRiskMetrics) as [string, {
    riskMultiplier: number;
    kellyBet: number;
    adjustedConfidence: number;
    consecutiveLosses: number;
    timestamp: number;
  }][];

  const botCount = bots.filter((b) => b.status === "running" || b.status === "idle").length;
  const trackedCount = metricEntries.length;

  if (trackedCount === 0) {
    return (
      <div className="rounded-lg border border-white/5 bg-white/[0.03] p-4 text-center">
        <Shield className="h-5 w-5 text-zinc-600 mx-auto mb-2" />
        <span className="text-xs text-zinc-500">
          Még nincsenek kockázati adatok. Amint a botok trade döntéseket hoznak, itt megjelennek a metrikák.
        </span>
      </div>
    );
  }

  // Aggregate calculations
  const avgRiskMultiplier =
    metricEntries.reduce((s, [, m]) => s + m.riskMultiplier, 0) / trackedCount;
  const avgAdjConfidence =
    metricEntries.reduce((s, [, m]) => s + m.adjustedConfidence, 0) / trackedCount;
  const totalKellyBet = metricEntries.reduce((s, [, m]) => s + m.kellyBet, 0);
  const totalConsecutiveLosses = metricEntries.reduce((s, [, m]) => s + m.consecutiveLosses, 0);
  const maxConsecutiveLosses = Math.max(...metricEntries.map(([, m]) => m.consecutiveLosses), 0);
  const maxConsecutiveBotId = metricEntries.find(
    ([, m]) => m.consecutiveLosses === maxConsecutiveLosses
  )?.[0];

  // Distribution of risk states
  const healthCount = metricEntries.filter(([, m]) => m.riskMultiplier >= 0.8).length;
  const warningCount = metricEntries.filter(
    ([, m]) => m.riskMultiplier >= 0.5 && m.riskMultiplier < 0.8
  ).length;
  const dangerCount = metricEntries.filter(([, m]) => m.riskMultiplier < 0.5).length;

  // Lowest risk multiplier bot
  const minRiskEntry = metricEntries.reduce((min, curr) =>
    curr[1].riskMultiplier < min[1].riskMultiplier ? curr : min
  );
  const minRiskBot = bots.find((b) => b.id === Number(minRiskEntry[0]));
  const minRiskBotName = minRiskBot?.name ?? `Bot #${minRiskEntry[0]}`;

  const latestUpdate = Math.max(...metricEntries.map(([, m]) => m.timestamp));

  const avgRiskColor =
    avgRiskMultiplier >= 0.8
      ? "text-green-400"
      : avgRiskMultiplier >= 0.5
        ? "text-amber-400"
        : "text-red-400";

  return (
    <motion.div
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-3"
    >
      {/* Summary cards row */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
        {/* Avg Risk Multiplier */}
        <div className="rounded-lg bg-orange-500/[0.04] border border-orange-500/10 px-2.5 py-2">
          <span className="text-[9px] uppercase text-zinc-500 font-semibold tracking-wider">
            Avg Risk
          </span>
          <div className={`text-sm font-extrabold font-mono ${avgRiskColor}`}>
            ×{avgRiskMultiplier.toFixed(3)}
          </div>
        </div>

        {/* Avg Adj. Confidence */}
        <div className="rounded-lg bg-indigo-500/[0.04] border border-indigo-500/10 px-2.5 py-2">
          <span className="text-[9px] uppercase text-zinc-500 font-semibold tracking-wider">
            Avg Adj.Conf
          </span>
          <div className="text-sm font-extrabold font-mono text-indigo-400">
            {(avgAdjConfidence * 100).toFixed(1)}%
          </div>
        </div>

        {/* Total Kelly Bet */}
        <div className="rounded-lg bg-amber-500/[0.04] border border-amber-500/10 px-2.5 py-2">
          <span className="text-[9px] uppercase text-zinc-500 font-semibold tracking-wider">
            Total Kelly
          </span>
          <div className="text-sm font-extrabold font-mono text-amber-400">
            ${totalKellyBet.toFixed(2)}
          </div>
        </div>

        {/* Total Consecutive Losses */}
        <div className="rounded-lg bg-rose-500/[0.04] border border-rose-500/10 px-2.5 py-2">
          <span className="text-[9px] uppercase text-zinc-500 font-semibold tracking-wider">
            Loss Streaks
          </span>
          <div
            className={`text-sm font-extrabold font-mono ${
              totalConsecutiveLosses === 0
                ? "text-green-400"
                : totalConsecutiveLosses <= 5
                  ? "text-amber-400"
                  : "text-red-400"
            }`}
          >
            {totalConsecutiveLosses} total
          </div>
        </div>
      </div>

      {/* Risk distribution bar */}
      <div className="flex items-center gap-3">
        <span className="text-[9px] uppercase text-zinc-600 font-bold tracking-wider whitespace-nowrap">
          Risk Dist.
        </span>
        <div className="flex-1 flex h-2 rounded-full bg-zinc-800 overflow-hidden">
          {healthCount > 0 && (
            <div
              className="bg-green-500/60 transition-all duration-500"
              style={{ width: `${(healthCount / trackedCount) * 100}%` }}
              title={`${healthCount} healthy`}
            />
          )}
          {warningCount > 0 && (
            <div
              className="bg-amber-500/60 transition-all duration-500"
              style={{ width: `${(warningCount / trackedCount) * 100}%` }}
              title={`${warningCount} warning`}
            />
          )}
          {dangerCount > 0 && (
            <div
              className="bg-red-500/60 transition-all duration-500"
              style={{ width: `${(dangerCount / trackedCount) * 100}%` }}
              title={`${dangerCount} danger`}
            />
          )}
        </div>
        <div className="flex items-center gap-2 text-[9px] font-mono">
          <span className="text-green-400">{healthCount}</span>
          <span className="text-amber-400">{warningCount}</span>
          <span className="text-red-400">{dangerCount}</span>
        </div>
      </div>

      {/* Alert for worst bot */}
      {minRiskEntry[1].riskMultiplier < 0.5 && (
        <motion.div
          initial={{ opacity: 0, x: -8 }}
          animate={{ opacity: 1, x: 0 }}
          className="flex items-center gap-2 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2"
        >
          <AlertTriangle className="h-3.5 w-3.5 text-red-400 shrink-0" />
          <span className="text-[11px] text-red-300">
            <strong className="text-red-200">{minRiskBotName}</strong> kritikus kockázati szinten
            (×{minRiskEntry[1].riskMultiplier.toFixed(3)})
          </span>
        </motion.div>
      )}

      {/* Consecutive loss alert */}
      {maxConsecutiveLosses >= 3 && maxConsecutiveBotId && (
        <motion.div
          initial={{ opacity: 0, x: -8 }}
          animate={{ opacity: 1, x: 0 }}
          className="flex items-center gap-2 rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2"
        >
          <TrendingDown className="h-3.5 w-3.5 text-rose-400 shrink-0" />
          <span className="text-[11px] text-rose-300">
            <strong className="text-rose-200">
              {bots.find((b) => b.id === Number(maxConsecutiveBotId))?.name ??
                `Bot #${maxConsecutiveBotId}`}
            </strong>{" "}
            {maxConsecutiveLosses >= 5
              ? "elérte a maximális veszteségsorozatot (5+)"
              : `${maxConsecutiveLosses} veszteség sorozatban`}
          </span>
        </motion.div>
      )}

      {/* Expandable per-bot detail table */}
      <div className="rounded-lg border border-white/5 bg-white/[0.02] overflow-hidden">
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center justify-between px-3 py-2 hover:bg-white/[0.02] transition-colors cursor-pointer"
        >
          <div className="flex items-center gap-2">
            <Bot className="h-3.5 w-3.5 text-zinc-500" />
            <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
              Per-Bot Details ({trackedCount})
            </span>
          </div>
          {expanded ? (
            <ChevronUp className="h-3.5 w-3.5 text-zinc-500" />
          ) : (
            <ChevronDown className="h-3.5 w-3.5 text-zinc-500" />
          )}
        </button>

        {expanded && (
          <div className="border-t border-white/5">
            {/* Table header */}
            <div className="grid grid-cols-[1fr_60px_70px_70px_80px] gap-2 px-3 py-1.5 bg-white/[0.02] text-[8px] uppercase font-bold tracking-wider text-zinc-600">
              <span>Bot</span>
              <span className="text-right">Risk</span>
              <span className="text-right">Adj.Conf</span>
              <span className="text-right">Kelly</span>
              <span className="text-right">Losses</span>
            </div>

            {metricEntries.map(([botId, metrics]) => {
              const bot = bots.find((b) => b.id === Number(botId));
              const riskColor =
                metrics.riskMultiplier >= 0.8
                  ? "text-green-400"
                  : metrics.riskMultiplier >= 0.5
                    ? "text-amber-400"
                    : "text-red-400";
              return (
                <div
                  key={botId}
                  className="grid grid-cols-[1fr_60px_70px_70px_80px] gap-2 px-3 py-1.5 border-t border-white/[0.03] hover:bg-white/[0.02] transition-colors text-[10px]"
                >
                  <span className="truncate font-medium text-zinc-300">
                    {bot?.name ?? `Bot #${botId}`}
                  </span>
                  <span className={`text-right font-bold font-mono ${riskColor}`}>
                    ×{metrics.riskMultiplier.toFixed(2)}
                  </span>
                  <span className="text-right font-mono text-indigo-400">
                    {(metrics.adjustedConfidence * 100).toFixed(0)}%
                  </span>
                  <span className="text-right font-mono text-amber-400">
                    ${metrics.kellyBet.toFixed(1)}
                  </span>
                  <span
                    className={`text-right font-mono ${
                      metrics.consecutiveLosses === 0
                        ? "text-green-400"
                        : metrics.consecutiveLosses <= 2
                          ? "text-amber-400"
                          : "text-red-400"
                    }`}
                  >
                    {metrics.consecutiveLosses}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Footer: last updated timestamp */}
      <div className="flex items-center gap-4 text-[9px] text-zinc-600">
        <span>
          {trackedCount} bot{trackedCount !== 1 ? "s" : ""} tracked · {botCount} total
        </span>
        <span>Utolsó frissítés: {formatTime(latestUpdate)}</span>
        <div className="flex items-center gap-1.5 ml-auto">
          {healthCount > 0 && (
            <span className="flex items-center gap-1">
              <ShieldCheck className="h-3 w-3 text-green-400" />
              <span className="text-green-400">{healthCount}</span>
            </span>
          )}
          {warningCount > 0 && (
            <span className="flex items-center gap-1">
              <ShieldAlert className="h-3 w-3 text-amber-400" />
              <span className="text-amber-400">{warningCount}</span>
            </span>
          )}
          {dangerCount > 0 && (
            <span className="flex items-center gap-1">
              <Shield className="h-3 w-3 text-red-400" />
              <span className="text-red-400">{dangerCount}</span>
            </span>
          )}
        </div>
      </div>
    </motion.div>
  );
}
