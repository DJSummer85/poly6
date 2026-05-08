"use client";

import { motion } from "framer-motion";
import type { ReactNode } from "react";

interface ErrorStateProps {
  title?: string;
  description?: string;
  onRetry?: () => void;
  icon?: ReactNode;
}

export function ErrorState({
  title = "Something went wrong",
  description = "An unexpected error occurred. Please try again.",
  onRetry,
  icon,
}: ErrorStateProps) {
  const displayIcon = icon ?? (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      aria-label="Error icon"
    >
      <circle cx="12" cy="12" r="10" />
      <path d="M12 8v4M12 16h.01" />
      <path d="M8.5 8.5l7 7M15.5 8.5l-7 7" strokeOpacity="0.3" />
    </svg>
  );

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
        className="text-neon-red mb-4"
      >
        {displayIcon}
      </motion.div>

      <h3 className="text-lg font-semibold text-text-primary mb-2">{title}</h3>

      {description && <p className="text-sm text-text-secondary max-w-xs mb-4">{description}</p>}

      {onRetry && (
        <button type="button" onClick={onRetry} className="btn-primary flex items-center gap-2">
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            aria-label="Retry"
          >
            <path d="M1 4v6h6" />
            <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
          </svg>
          Try Again
        </button>
      )}
    </motion.div>
  );
}
