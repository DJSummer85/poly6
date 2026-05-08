"use client";

import { motion } from "framer-motion";
import { cn } from "@/lib/utils";

interface GlassCardProps {
  children: React.ReactNode;
  className?: string;
  hover?: boolean;
  animate?: boolean;
}

export function GlassCard({ children, className, hover, animate }: GlassCardProps) {
  const Component = animate ? motion.div : "div";
  const animationProps = animate
    ? { initial: { opacity: 0, y: 10 }, animate: { opacity: 1, y: 0 } }
    : {};

  return (
    <Component
      {...animationProps}
      className={cn("glass-card", hover && "glass-card-hover", className)}
    >
      {children}
    </Component>
  );
}

export function Skeleton({ className }: { className?: string }) {
  return <div className={cn("skeleton rounded-lg", className)} />;
}
