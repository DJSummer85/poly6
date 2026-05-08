"use client";

import { motion } from "framer-motion";
import type { ReactNode } from "react";

type EmptyStateVariant = "bot" | "trade" | "data" | "search" | "general";

interface EmptyStateProps {
  variant?: EmptyStateVariant;
  title: string;
  description?: string;
  action?: ReactNode;
  icon?: ReactNode;
}

const variantIcons: Record<EmptyStateVariant, ReactNode> = {
  bot: (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-label="Bot icon"
    >
      <rect x="3" y="11" width="18" height="10" rx="2" />
      <circle cx="8.5" cy="15.5" r="1.5" />
      <circle cx="15.5" cy="15.5" r="1.5" />
      <path d="M9 7h6M9 7v-2M15 7v-2" />
    </svg>
  ),
  trade: (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-label="Trade icon"
    >
      <path d="M7 16V4m0 0L3 8m4-4l4 4" />
      <path d="M17 8v12m0 0l4-4m-4 4l-4-4" />
    </svg>
  ),
  data: (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-label="Data icon"
    >
      <ellipse cx="12" cy="5" rx="9" ry="3" />
      <path d="M3 5v6c0 5 4 8 9 8s9-3 9-8V5" />
      <path d="M3 11v6c0 5 4 8 9 8s9-3 9-8v-6" />
    </svg>
  ),
  search: (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-label="Search icon"
    >
      <circle cx="11" cy="11" r="8" />
      <path d="M21 21l-4.35-4.35" />
    </svg>
  ),
  general: (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-label="Info icon"
    >
      <circle cx="12" cy="12" r="10" />
      <path d="M12 8v4M12 16h.01" />
    </svg>
  ),
};

export function EmptyState({
  variant = "general",
  title,
  description,
  action,
  icon,
}: EmptyStateProps) {
  const displayIcon = icon ?? variantIcons[variant];

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: "easeOut" }}
      className="flex flex-col items-center justify-center py-12 px-4 text-center"
    >
      <motion.div
        initial={{ scale: 0.8 }}
        animate={{ scale: 1 }}
        transition={{ duration: 0.4, ease: "easeOut", delay: 0.1 }}
        className="text-text-muted mb-4"
      >
        {displayIcon}
      </motion.div>

      <h3 className="text-lg font-semibold text-text-primary mb-2">{title}</h3>

      {description && <p className="text-sm text-text-secondary max-w-xs mb-4">{description}</p>}

      {action && <div className="mt-2">{action}</div>}
    </motion.div>
  );
}
