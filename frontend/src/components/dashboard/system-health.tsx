"use client";

import { motion } from "framer-motion";
import { Activity, Clock, Database, Server, Shield, Wifi, Zap } from "lucide-react";
import { useSystemStatus } from "@/hooks";
import { useAppStore } from "@/store";

export function SystemHealth() {
  const { data: sys, isLoading } = useSystemStatus();
  const latency = useAppStore((s) => s.latency);
  const systemStatus = useAppStore((s) => s.systemStatus);

  // Latency sparkline (last 30 samples)
  const sparkline = latency.samples.slice(-30);
  const maxSpark = Math.max(...sparkline, 1);

  const formatTime = (ts: number) =>
    new Date(ts).toLocaleTimeString("hu-HU", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });

  const healthItems = [
    {
      label: "SSE Connection",
      icon: Wifi,
      status: "healthy",
      detail: `${latency.avg.toFixed(1)}ms avg JS latency`,
    },
    {
      label: "Backend Status",
      icon: Server,
      status: isLoading ? "loading" : sys ? "healthy" : "unknown",
      detail: sys ? `${sys.running_bots} bots running` : "Connecting...",
    },
    {
      label: "Binance API",
      icon: Activity,
      status: sys?.binance_connected ? "healthy" : sys ? "disconnected" : "unknown",
      detail: sys?.binance_connected ? "Connected" : "Not connected",
    },
    {
      label: "Polymarket CLOB",
      icon: Shield,
      status: sys?.has_polymarket_credentials ? "configured" : "unknown",
      detail: sys?.has_polymarket_credentials ? "Configured" : "No credentials",
    },
    {
      label: "Database",
      icon: Database,
      status: "healthy",
      detail: `${systemStatus?.bots_total ?? 0} bots tracked`,
    },
  ];

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Status items */}
      <div className="space-y-2">
        {healthItems.map((item) => {
          const Icon = item.icon;
          const statusColor =
            item.status === "healthy" || item.status === "configured"
              ? "text-green-400"
              : item.status === "loading"
                ? "text-amber-400"
                : "text-zinc-600";

          const dotColor =
            item.status === "healthy" || item.status === "configured"
              ? "bg-green-400"
              : item.status === "loading"
                ? "bg-amber-400 animate-pulse"
                : "bg-zinc-600";

          return (
            <motion.div
              key={item.label}
              initial={{ opacity: 0, x: -8 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex items-center justify-between rounded-lg border border-white/5 bg-white/[0.02] px-3 py-2.5"
            >
              <div className="flex items-center gap-3">
                <div
                  className={`flex h-8 w-8 items-center justify-center rounded-lg ${statusColor}/15`}
                >
                  <Icon className={`h-4 w-4 ${statusColor}`} />
                </div>
                <div>
                  <div className="text-sm font-semibold text-zinc-200">{item.label}</div>
                  <div className="text-[10px] text-zinc-500">{item.detail}</div>
                </div>
              </div>

              <div className="flex items-center gap-1.5">
                <div className={`h-2 w-2 rounded-full ${dotColor}`} />
                <span className={`text-[10px] font-medium capitalize ${statusColor}`}>
                  {item.status === "loading" ? "..." : item.status}
                </span>
              </div>
            </motion.div>
          );
        })}
      </div>

      {/* Latency chart */}
      {sparkline.length > 1 && (
        <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <Zap className="h-3.5 w-3.5 text-zinc-500" />
              <span className="text-xs font-semibold text-zinc-300">JS Processing Latency</span>
            </div>
            <div className="flex items-center gap-3 text-[10px] font-mono text-zinc-500">
              <span>min: {latency.min.toFixed(1)}ms</span>
              <span>avg: {latency.avg.toFixed(1)}ms</span>
              <span>max: {latency.max.toFixed(1)}ms</span>
            </div>
          </div>

          <svg
            width="100%"
            height="40"
            className="overflow-visible"
            role="img"
            aria-label="Latency chart"
          >
            {(() => {
              const w = 300; // viewBox width
              const h = 40;
              const points = sparkline.map((v, i) => ({
                x: (i / (sparkline.length - 1)) * w,
                y: h - (v / maxSpark) * (h - 4),
                color: v < 0.5 ? "#22c55e" : v < 1.0 ? "#f59e0b" : "#ef4444",
              }));

              const linePath = points
                .map((p, i) => `${i === 0 ? "M" : "L"}${p.x},${p.y}`)
                .join(" ");
              const areaPath = `${linePath} L${w},${h} L0,${h} Z`;

              return (
                <g>
                  <defs>
                    <linearGradient id="latencyGradient" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#6366f1" stopOpacity="0.3" />
                      <stop offset="100%" stopColor="#6366f1" stopOpacity="0" />
                    </linearGradient>
                  </defs>
                  <path d={areaPath} fill="url(#latencyGradient)" />
                  <path d={linePath} fill="none" stroke="#6366f1" strokeWidth="1.5" />
                  {points.map((p) => (
                    <circle
                      key={`sp-${p.x.toFixed(1)}-${p.y.toFixed(1)}`}
                      cx={p.x}
                      cy={p.y}
                      r="1.5"
                      fill={p.color}
                    />
                  ))}
                </g>
              );
            })()}
          </svg>
        </div>
      )}

      {/* Bot fleet summary */}
      {systemStatus && (
        <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3">
          <div className="flex items-center gap-2 mb-2">
            <Clock className="h-3.5 w-3.5 text-zinc-500" />
            <span className="text-xs font-semibold text-zinc-300">Bot Fleet</span>
          </div>
          <div className="grid grid-cols-3 gap-3">
            <div>
              <div className="text-[10px] text-zinc-500">Total</div>
              <div className="text-base font-extrabold font-mono text-zinc-100">
                {systemStatus.bots_total}
              </div>
            </div>
            <div>
              <div className="text-[10px] text-zinc-500">Running</div>
              <div className="text-base font-extrabold font-mono text-green-400">
                {systemStatus.bots_running}
              </div>
            </div>
            <div>
              <div className="text-[10px] text-zinc-500">Idle</div>
              <div className="text-base font-extrabold font-mono text-zinc-400">
                {systemStatus.bots_total - systemStatus.bots_running}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Last update */}
      {systemStatus?.last_update && (
        <div className="text-center text-[10px] text-zinc-600">
          Last update: {formatTime(systemStatus.last_update)}
        </div>
      )}
    </div>
  );
}
