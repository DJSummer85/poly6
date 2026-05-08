"use client";

import { useEffect, useState } from "react";
import { Header } from "@/components/layout/header";
import { Sidebar } from "@/components/layout/sidebar";
import { AmbientGlow } from "@/components/ui/ambient-glow";
import { useSSE } from "@/hooks";
import { useAppStore } from "@/store";

interface AppShellProps {
  children: React.ReactNode;
}

export function AppShell({ children }: AppShellProps) {
  const { sidebarCollapsed } = useAppStore();

  // SSE connection for real-time data
  useSSE();

  const [isMounted, setIsMounted] = useState(false);
  useEffect(() => {
    setIsMounted(true);
  }, []);

  if (!isMounted) {
    return (
      <div className="min-h-screen bg-background relative overflow-hidden">
        <AmbientGlow color="green" position="top-left" />
        <AmbientGlow color="blue" position="bottom-right" />
        <AmbientGlow color="primary" position="center" />
        <div className="flex h-screen items-center justify-center">
          <div className="flex flex-col items-center gap-4">
            <div className="h-12 w-12 animate-spin rounded-full border-4 border-indigo-500 border-t-transparent" />
            <p className="text-sm font-medium text-zinc-400">Betöltés...</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background relative overflow-hidden">
      <AmbientGlow color="green" position="top-left" />
      <AmbientGlow color="blue" position="bottom-right" />
      <AmbientGlow color="primary" position="center" />

      <div className="flex h-screen overflow-hidden">
        <Sidebar collapsed={sidebarCollapsed} />
        <div className="flex-1 flex flex-col overflow-hidden min-w-0">
          <Header />
          <main className="flex-1 overflow-auto">
            <div className="max-w-7xl mx-auto p-4 lg:p-6">
              {children}
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}