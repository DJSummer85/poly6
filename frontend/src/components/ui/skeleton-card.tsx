"use client";

import { motion } from "framer-motion";

type SkeletonVariant = "card" | "text" | "circular" | "bot-row" | "trade-row";

interface SkeletonCardProps {
  variant?: SkeletonVariant;
  className?: string;
  count?: number;
}

export function SkeletonCard({ variant = "card", className = "", count = 1 }: SkeletonCardProps) {
  const items = Array.from({ length: count }, (_, i) => i);

  if (variant === "bot-row") {
    return (
      <div className="space-y-2">
        {items.map((i) => (
          <motion.div
            key={i}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.3, delay: i * 0.05 }}
            className="flex items-center gap-3 p-3 rounded-lg bg-glass-bg"
          >
            <div className="skeleton w-10 h-10 rounded-lg" />
            <div className="flex-1 space-y-2">
              <div className="skeleton h-4 w-3/4 rounded" />
              <div className="skeleton h-3 w-1/2 rounded" />
            </div>
            <div className="skeleton w-16 h-6 rounded-full" />
          </motion.div>
        ))}
      </div>
    );
  }

  if (variant === "trade-row") {
    return (
      <div className="space-y-2">
        {items.map((i) => (
          <motion.div
            key={i}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.3, delay: i * 0.05 }}
            className="flex items-center gap-4 p-3 rounded-lg"
          >
            <div className="skeleton w-8 h-8 rounded-full" />
            <div className="skeleton w-20 h-4 rounded" />
            <div className="skeleton w-16 h-4 rounded" />
            <div className="skeleton w-12 h-4 rounded ml-auto" />
          </motion.div>
        ))}
      </div>
    );
  }

  if (variant === "text") {
    return (
      <div className="space-y-2">
        {items.map((i) => (
          <motion.div
            key={i}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.3, delay: i * 0.05 }}
            className={`space-y-2 ${className}`}
          >
            <div className="skeleton h-4 w-full rounded" />
            <div className="skeleton h-4 w-4/5 rounded" />
            <div className="skeleton h-4 w-3/5 rounded" />
          </motion.div>
        ))}
      </div>
    );
  }

  if (variant === "circular") {
    return (
      <div className="flex items-center gap-3">
        {items.map((i) => (
          <motion.div
            key={i}
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ duration: 0.3, delay: i * 0.05 }}
            className="skeleton rounded-full"
            style={{ width: 40, height: 40 }}
          />
        ))}
      </div>
    );
  }

  // Default: card variant
  return (
    <div className="space-y-2">
      {items.map((i) => (
        <motion.div
          key={i}
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, delay: i * 0.08 }}
          className={`glass-card p-4 ${className}`}
        >
          <div className="flex items-start gap-3">
            <div className="skeleton w-12 h-12 rounded-lg" />
            <div className="flex-1 space-y-2">
              <div className="skeleton h-5 w-3/4 rounded" />
              <div className="skeleton h-4 w-1/2 rounded" />
            </div>
          </div>
          <div className="mt-4 space-y-2">
            <div className="skeleton h-3 w-full rounded" />
            <div className="skeleton h-3 w-5/6 rounded" />
          </div>
        </motion.div>
      ))}
    </div>
  );
}
