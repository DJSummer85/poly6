"use client";

import { motion } from "framer-motion";
import type { ReactNode } from "react";
import { cn, formatPercent, formatPrice } from "@/lib/utils";

interface StatCardProps {
  label: string;
  value: number;
  suffix?: string;
  format?: "price" | "percent" | "pnl" | "number";
  icon?: ReactNode;
  highlight?: boolean;
}

export function StatCard({
  label,
  value,
  suffix,
  format = "number",
  icon,
  highlight,
}: StatCardProps) {
  const formattedValue =
    format === "price"
      ? formatPrice(value)
      : format === "percent"
        ? formatPercent(value)
        : format === "pnl"
          ? `${value >= 0 ? "+" : ""}${value.toFixed(2)}`
          : value.toString();

  const valueColor =
    format === "pnl"
      ? value >= 0
        ? "text-neon-green"
        : "text-neon-red"
      : highlight
        ? "text-btc"
        : "text-text";

  return (
    <motion.div
      whileHover={{ scale: 1.02 }}
      className={cn("glass-card px-4 py-3 flex items-center gap-3", highlight && "border-btc/30")}
    >
      {icon && <div className="shrink-0">{icon}</div>}
      <div className="flex flex-col">
        <span className="text-xs text-text-muted uppercase tracking-wide">{label}</span>
        <div className="flex items-baseline gap-1">
          <motion.span
            key={value}
            initial={{ scale: 1 }}
            animate={{ scale: [1, 1.05, 1] }}
            transition={{ duration: 0.3 }}
            className={cn("font-mono font-bold text-lg", valueColor)}
          >
            {formattedValue}
          </motion.span>
          {suffix && <span className="text-xs text-text-muted">{suffix}</span>}
        </div>
      </div>
    </motion.div>
  );
}
