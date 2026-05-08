"use client";

import { motion } from "framer-motion";
import {
  ArrowUpRight,
  Bot,
  LineChart,
  Loader2,
  Search,
  Target,
  TrendingDown,
  TrendingUp,
  XCircle,
} from "lucide-react";
import { useAppStore } from "@/store";

const EMPTY_ACTIVITIES: never[] = [];

const ACTIVITY_CONFIG: Record<string, { icon: typeof Bot; color: string; label: string }> = {
  scanning: { icon: Search, color: "text-zinc-400", label: "Scanning market" },
  evaluating: { icon: LineChart, color: "text-blue-400", label: "Evaluating strategy" },
  trade_decision: { icon: Target, color: "text-amber-400", label: "Trade decision" },
  order_executed: { icon: ArrowUpRight, color: "text-green-400", label: "Order executed" },
  position_update: { icon: TrendingUp, color: "text-cyan-400", label: "Position update" },
  trade_result: { icon: TrendingDown, color: "text-violet-400", label: "Trade result" },
  error: { icon: XCircle, color: "text-red-400", label: "Error" },
};

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleTimeString("hu-HU", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function ActivityIcon({ type }: { type: string }) {
  const config = ACTIVITY_CONFIG[type] ?? { icon: Bot, color: "text-zinc-500", label: type };
  const Icon = config.icon;
  return <Icon className={`h-3.5 w-3.5 ${config.color}`} />;
}

function ActivityItem({
  activity,
}: {
  activity: {
    id: string;
    botId: number;
    type: string;
    timestamp: number;
    data: Record<string, unknown>;
  };
}) {
  const config = ACTIVITY_CONFIG[activity.type] ?? { label: activity.type };

  return (
    <motion.div
      initial={{ opacity: 0, x: -8 }}
      animate={{ opacity: 1, x: 0 }}
      className="flex items-start gap-2 px-2 py-1.5 rounded-md hover:bg-white/[0.03] transition-colors"
    >
      <div className="mt-0.5 shrink-0">
        <ActivityIcon type={activity.type} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between">
          <span className="text-xs font-medium text-zinc-300">{config.label}</span>
          <span className="text-[10px] font-mono text-zinc-600">
            {formatTime(activity.timestamp)}
          </span>
        </div>
        <ActivityDetail activity={activity} />
      </div>
    </motion.div>
  );
}

function ActivityDetail({
  activity,
}: {
  activity: { type: string; data: Record<string, unknown> };
}) {
  const { type, data } = activity;

  if (type === "trade_decision") {
    const side = data.outcome as string;
    const size = data.betSize as number;
    const conf = data.confidence as number;
    return (
      <div className="flex items-center gap-2 mt-0.5">
        <span
          className={`text-[10px] font-bold px-1 rounded ${
            side === "YES" ? "bg-green-500/15 text-green-400" : "bg-red-500/15 text-red-400"
          }`}
        >
          {side}
        </span>
        <span className="text-[10px] text-zinc-500">
          ${typeof size === "number" ? size.toFixed(2) : "—"}
        </span>
        <span className="text-[10px] text-zinc-600">
          Confidence: {typeof conf === "number" ? (conf * 100).toFixed(0) : "—"}%
        </span>
      </div>
    );
  }

  if (type === "order_executed") {
    return (
      <span className="text-[10px] font-mono text-zinc-600">
        ID: {typeof data.orderId === "string" ? data.orderId.slice(0, 12) : "—"}
      </span>
    );
  }

  if (type === "position_update") {
    const side = data.side as string;
    const price = data.price as number;
    const pnl = data.unrealizedPnl as number;
    return (
      <div className="flex items-center gap-2 mt-0.5">
        {side && (
          <span
            className={`text-[10px] font-bold px-1 rounded ${
              side === "YES" ? "bg-green-500/15 text-green-400" : "bg-red-500/15 text-red-400"
            }`}
          >
            {side}
          </span>
        )}
        {typeof price === "number" && (
          <span className="text-[10px] text-zinc-500">@ {price.toFixed(3)}</span>
        )}
        {typeof pnl === "number" && (
          <span className={`text-[10px] font-mono ${pnl >= 0 ? "text-green-400" : "text-red-400"}`}>
            {pnl >= 0 ? "+" : ""}${pnl.toFixed(2)}
          </span>
        )}
      </div>
    );
  }

  if (type === "trade_result") {
    const won = data.won as boolean;
    const pnl = data.pnl as number;
    return (
      <div className="flex items-center gap-2 mt-0.5">
        <span className={`text-[10px] font-bold ${won ? "text-green-400" : "text-red-400"}`}>
          {won ? "WIN" : "LOSS"}
        </span>
        {typeof pnl === "number" && (
          <span className={`text-[10px] font-mono ${pnl >= 0 ? "text-green-400" : "text-red-400"}`}>
            {pnl >= 0 ? "+" : ""}${pnl.toFixed(2)}
          </span>
        )}
      </div>
    );
  }

  if (type === "evaluating") {
    const strategy = data.strategy as string;
    const conf = data.confidence as number;
    return (
      <div className="flex items-center gap-2 mt-0.5">
        {strategy && <span className="text-[10px] text-zinc-500">{strategy}</span>}
        {typeof conf === "number" && (
          <span className="text-[10px] text-zinc-600">Confidence: {(conf * 100).toFixed(0)}%</span>
        )}
      </div>
    );
  }

  if (type === "error") {
    return (
      <span className="text-[10px] text-red-400/80">
        {typeof data.message === "string" ? data.message : "Unknown error"}
      </span>
    );
  }

  if (type === "scanning") {
    return (
      <span className="text-[10px] text-zinc-600">
        {typeof data.market === "string" ? data.market : "Looking for opportunities..."}
      </span>
    );
  }

  return null;
}

export function LiveBotActivityCard({ botId }: { botId: number }) {
  const activities = useAppStore((s) => s.botActivities[botId] ?? EMPTY_ACTIVITIES);

  if (activities.length === 0) {
    return (
      <div className="rounded-lg bg-white/3 border border-white/5 px-3 py-4 text-center">
        <Loader2 className="h-4 w-4 text-zinc-600 animate-spin mx-auto mb-1.5" />
        <span className="text-[10px] text-zinc-500">Bot inicializálása...</span>
      </div>
    );
  }

  const lastActivity = activities[activities.length - 1];
  const lastConfig = ACTIVITY_CONFIG[lastActivity.type] ?? { label: lastActivity.type };
  const LastIcon = lastConfig.icon;

  return (
    <div className="rounded-lg bg-white/[0.03] border border-white/8 overflow-hidden">
      {/* Current status header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-white/5 bg-white/[0.02]">
        <LastIcon className={`h-3.5 w-3.5 ${lastConfig.color}`} />
        <span className="text-[10px] font-semibold text-zinc-400 uppercase tracking-wider">
          {lastConfig.label}
        </span>
        <span className="text-[10px] text-zinc-600 ml-auto font-mono">
          {formatTime(lastActivity.timestamp)}
        </span>
      </div>

      {/* Activity feed */}
      <div className="max-h-48 overflow-y-auto py-1">
        {[...activities].reverse().map((activity) => (
          <ActivityItem key={activity.id} activity={activity} />
        ))}
      </div>
    </div>
  );
}
