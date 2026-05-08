"use client";

import { motion } from "framer-motion";
import { Bitcoin } from "lucide-react";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/store";

export function PriceTicker() {
  const { btcPrice } = useAppStore();

  const formatPrice = (p: number) =>
    p > 0
      ? `$${p.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`
      : "---";

  return (
    <motion.div className={cn("glass-card px-4 py-2 flex items-center gap-2", "border-btc/30")}>
      <Bitcoin className="w-5 h-5 text-btc" />
      <motion.span
        key={btcPrice}
        initial={{ scale: 1 }}
        animate={{
          scale: [1, 1.1, 1],
          color: btcPrice > 0 ? ["#fafafa", "#22c55e", "#fafafa"] : "#fafafa",
        }}
        transition={{ duration: 0.4 }}
        className="font-mono font-bold text-lg text-text price-ticker"
      >
        {formatPrice(btcPrice)}
      </motion.span>
    </motion.div>
  );
}
