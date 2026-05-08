"use client";

import { motion } from "framer-motion";
import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import { cn } from "@/lib/utils";

type BadgeVariant = "success" | "error" | "warning" | "info";

interface StatusBadgeProps {
  variant: BadgeVariant;
  text: string;
  pulse?: boolean;
}

const variantConfig: Record<BadgeVariant, { bg: string; text: string; icon: React.ReactNode }> = {
  success: {
    bg: "bg-neon-green-muted",
    text: "text-neon-green",
    icon: <CheckCircle2 className="w-3 h-3" />,
  },
  error: {
    bg: "bg-neon-red-muted",
    text: "text-neon-red",
    icon: <XCircle className="w-3 h-3" />,
  },
  warning: {
    bg: "bg-orange-500/15",
    text: "text-orange-400",
    icon: <AlertTriangle className="w-3 h-3" />,
  },
  info: {
    bg: "bg-neon-blue-muted",
    text: "text-neon-blue",
    icon: <Info className="w-3 h-3" />,
  },
};

export function StatusBadge({ variant, text, pulse }: StatusBadgeProps) {
  const config = variantConfig[variant];

  return (
    <motion.span
      animate={pulse ? { scale: [1, 1.05, 1] } : {}}
      transition={{ repeat: pulse ? Infinity : 0, duration: 1.5 }}
      className={cn(
        "inline-flex items-center gap-1 px-2 py-1 rounded text-xs font-medium",
        config.bg,
        config.text
      )}
    >
      {config.icon}
      {text}
    </motion.span>
  );
}
