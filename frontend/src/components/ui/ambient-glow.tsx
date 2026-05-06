"use client";

import { cn } from "@/lib/utils";

interface AmbientGlowProps {
  color: "green" | "red" | "blue" | "primary";
  position: "top-left" | "top-right" | "bottom-left" | "bottom-right" | "center";
}

const colorMap = {
  green: "bg-neon-green",
  red: "bg-neon-red",
  blue: "bg-neon-blue",
  primary: "bg-primary",
};

const positionMap = {
  "top-left": "top-[-20%] left-[-10%] w-[600px] h-[600px]",
  "top-right": "top-[-20%] right-[-10%] w-[500px] h-[500px]",
  "bottom-left": "bottom-[-20%] left-[-10%] w-[500px] h-[500px]",
  "bottom-right": "bottom-[-20%] right-[-10%] w-[600px] h-[600px]",
  center: "top-[30%] left-[40%] w-[800px] h-[800px]",
};

export function AmbientGlow({ color, position }: AmbientGlowProps) {
  return <div className={cn("ambient-glow", colorMap[color], positionMap[position])} />;
}
