"use client";

import { motion, AnimatePresence } from "framer-motion";
import { Terminal, X, Brain, Trash2, Clock, Bot } from "lucide-react";
import { useEffect, useRef } from "react";
import { useAppStore } from "@/store";

interface BotThoughtsProps {
  isOpen: boolean;
  onClose: () => void;
}

export function BotThoughts({ isOpen, onClose }: BotThoughtsProps) {
  const { thoughts, clearThoughts } = useAppStore();
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [thoughts]);

  return (
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 md:p-8">
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="absolute inset-0 bg-black/90 backdrop-blur-md"
          />

          {/* Console Window */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            className="relative w-full max-w-5xl h-[80vh] flex flex-col bg-black border border-[#00ff41]/30 rounded-lg shadow-[0_0_30px_rgba(0,255,65,0.15)] overflow-hidden"
          >
            {/* Matrix Background Effect */}
            <div className="absolute inset-0 opacity-[0.03] pointer-events-none overflow-hidden select-none font-mono text-[10px] leading-none text-[#00ff41]">
              {Array.from({ length: 20 }).map((_, i) => (
                <div
                  key={i}
                  className="absolute top-0 animate-matrix-rain whitespace-pre"
                  style={{
                    left: `${i * 5}%`,
                    animationDelay: `${Math.random() * 5}s`,
                    animationDuration: `${5 + Math.random() * 10}s`,
                  }}
                >
                  {"0101010101\n1010101010\n0110011001\n1111000011\n0000111100\n".repeat(20)}
                </div>
              ))}
            </div>

            {/* Header */}
            <div className="flex items-center justify-between px-6 py-3 border-b border-[#00ff41]/20 bg-black/80 relative z-10">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-[#00ff41]/10 border border-[#00ff41]/30">
                  <Brain className="h-5 w-5 text-[#00ff41]" />
                </div>
                <div>
                  <h2 className="text-lg font-bold text-[#00ff41] tracking-tighter uppercase">Matrix Console // Bot Thinking</h2>
                  <p className="text-[10px] text-[#00ff41]/60 font-mono">NEURAL_LINK: ACTIVE // TRANSLATION_LAYER: HU_HU</p>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={clearThoughts}
                  className="p-2 text-[#00ff41]/40 hover:text-red-500 hover:bg-red-500/10 rounded transition-colors"
                  title="WIPE_MEMORY"
                >
                  <Trash2 className="h-5 w-5" />
                </button>
                <button
                  onClick={onClose}
                  className="p-2 text-[#00ff41]/40 hover:text-[#00ff41] hover:bg-[#00ff41]/10 rounded transition-colors"
                >
                  <X className="h-6 w-6" />
                </button>
              </div>
            </div>

            {/* Console Content */}
            <div
              ref={scrollRef}
              className="flex-1 overflow-y-auto p-4 font-mono text-[12px] space-y-0 bg-transparent relative z-10 custom-scrollbar"
            >
              <style jsx>{`
                .custom-scrollbar::-webkit-scrollbar { width: 4px; }
                .custom-scrollbar::-webkit-scrollbar-track { background: rgba(0, 255, 65, 0.05); }
                .custom-scrollbar::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.2); border-radius: 2px; }
                @keyframes matrix-rain {
                  from { transform: translateY(-100%); }
                  to { transform: translateY(100%); }
                }
                .animate-matrix-rain {
                  animation: matrix-rain linear infinite;
                }
              `}</style>
              
              {thoughts.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-[#00ff41]/20 gap-4">
                  <Terminal className="h-12 w-12 animate-pulse" />
                  <p className="text-xs tracking-[0.2em] uppercase">Várakozás a forráskódra...</p>
                </div>
              ) : (
                thoughts.map((t) => (
                  <motion.div
                    key={t.id}
                    initial={{ opacity: 0, x: -5 }}
                    animate={{ opacity: 1, x: 0 }}
                    className="group flex gap-3 py-0.5 border-b border-[#00ff41]/5 hover:bg-[#00ff41]/5 transition-colors px-2"
                  >
                    <div className="flex items-center gap-2 text-[#00ff41]/40 shrink-0 w-20">
                      <Clock className="h-2.5 w-2.5" />
                      <span className="text-[9px]">
                        {new Date(t.timestamp).toLocaleTimeString([], { hour12: false })}
                      </span>
                    </div>

                    <div className="flex items-center gap-2 shrink-0 w-28">
                      <Bot className="h-2.5 w-2.5 text-[#00ff41]/60" />
                      <span className="font-bold text-[#00ff41]/80 text-[10px] truncate uppercase tracking-tighter">
                        {t.botName}
                      </span>
                    </div>

                    <div
                      className={`flex-1 font-medium tracking-tight ${
                        t.type === "success"
                          ? "text-[#00ff41] brightness-125"
                          : t.type === "warn"
                            ? "text-amber-400/90"
                            : t.type === "error"
                              ? "text-red-500"
                              : "text-[#00ff41]/80"
                      }`}
                    >
                      <span className="mr-2 opacity-30 text-[#00ff41]">&gt;</span>
                      {t.text}
                    </div>
                  </motion.div>
                ))
              )}
            </div>

            {/* Footer */}
            <div className="px-6 py-2 border-t border-[#00ff41]/20 bg-black/80 flex items-center justify-between relative z-10">
              <div className="flex items-center gap-2">
                <div className="h-1.5 w-1.5 rounded-full bg-[#00ff41] animate-ping" />
                <span className="text-[9px] uppercase font-bold tracking-[0.3em] text-[#00ff41]/40">
                  Matrix Stream Connected
                </span>
              </div>
              <span className="text-[9px] font-mono text-[#00ff41]/40">
                LOG_COUNT: {thoughts.length.toString().padStart(4, '0')}
              </span>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
