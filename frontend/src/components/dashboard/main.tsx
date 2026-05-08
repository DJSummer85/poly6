"use client";

import { useEffect, useState } from "react";
import { AuthLanding } from "@/components/dashboard/auth-landing";
import { CommandCenter } from "@/components/dashboard/command-center";
import { Header } from "@/components/layout/header";
import { Sidebar } from "@/components/layout/sidebar";
import { AmbientGlow } from "@/components/ui/ambient-glow";
import { useBtcPrice, usePositions, useSSE, useUser } from "@/hooks";
import { useAppStore } from "@/store";

export function Dashboard() {
  const { token, isAuthenticated, user, setAuth, setMarketData, setPositions } = useAppStore();
  const { data: userData } = useUser();

  // Sync user data from API to store (fixes missing user state after login)
  useEffect(() => {
    if (userData && (!user || user.username !== userData.username)) {
      // Only update user data, don't overwrite token from localStorage
      const storedToken = typeof window !== "undefined" ? localStorage.getItem("token") : null;
      if (storedToken) {
        setAuth(storedToken, { id: userData.id, email: "", username: userData.username });
      }
    }
  }, [userData, user, setAuth]);

  // SSE connection for real-time data
  useSSE();

  // Fetch initial data via API (only when authenticated)
  // NOTE: We no longer auto-load bots into store - bots are managed via /bots page
  // and manual selection on the dashboard
  const { data: btcData } = useBtcPrice();
  const { data: positionsData } = usePositions();

  // Sync API data to store
  useEffect(() => {
    if (btcData?.price) setMarketData({ btcPrice: btcData.price });
  }, [btcData, setMarketData]);

  useEffect(() => {
    if (positionsData) setPositions(positionsData);
  }, [positionsData, setPositions]);

  const { sidebarCollapsed } = useAppStore();

  // Wait for mount to check localStorage - prevents hydration mismatch
  const [isMounted, setIsMounted] = useState(false);
  useEffect(() => {
    setIsMounted(true);
  }, []);

  const isAuthed =
    isMounted &&
    (isAuthenticated ||
      !!token ||
      (typeof window !== "undefined" && !!localStorage.getItem("token")));

  // Show loading state until mounted - prevents hydration mismatch
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
      {/* Ambient glow effects */}
      <AmbientGlow color="green" position="top-left" />
      <AmbientGlow color="blue" position="bottom-right" />
      <AmbientGlow color="primary" position="center" />

      {/* Layout */}
      <div className="flex h-screen overflow-hidden">
        {/* Sidebar */}
        <Sidebar collapsed={sidebarCollapsed} />

        {/* Main content */}
        <div className="flex-1 flex flex-col overflow-hidden min-w-0">
          <Header />
          <main className="flex-1 overflow-auto">
            <div className="max-w-7xl mx-auto p-4 lg:p-6">
              {isAuthed ? <CommandCenter /> : <AuthLanding />}
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
