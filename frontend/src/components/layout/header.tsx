"use client";

import { LogIn, LogOut, Settings, User, Wallet } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { useSettings } from "@/hooks";
import { useAppStore } from "@/store";

export function Header() {
  const router = useRouter();
  const { clearAuth, user, isAuthenticated } = useAppStore();
  const { data: settings } = useSettings();
  const hasCreds = settings?.has_credentials ?? false;

  const handleLogout = () => {
    clearAuth();
    toast.success("Sikeres kijelentkezés");
    router.push("/");
  };

  return (
    <header className="border-b border-white/8 bg-zinc-950/95 backdrop-blur-xl">
      <div className="flex items-center justify-between gap-4 mx-auto max-w-7xl py-3 px-6">
        {/* Left: Logo */}
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-indigo-500/15">
            <Settings className="h-4 w-4 text-indigo-500" />
          </div>
          <span className="text-lg font-bold text-zinc-100">
            Poly<span className="text-indigo-500">Trade</span>
          </span>
        </div>

        {/* Center: Status Chips */}
        <div className="hidden items-center gap-2 md:flex">
          {/* Auth Status */}
          {isAuthenticated && user ? (
            <div className="flex items-center gap-2 rounded-lg bg-green-500/10 border border-green-500/20 px-3 py-1.5">
              <div className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
              <User className="h-3 w-3 text-green-400" />
              <span className="text-xs font-semibold text-green-400">@{user.username}</span>
            </div>
          ) : (
            <Link
              href="/login"
              className="flex items-center gap-2 rounded-lg bg-zinc-800/50 border border-white/10 px-3 py-1.5 hover:bg-zinc-800/70 transition-colors"
            >
              <div className="h-2 w-2 rounded-full bg-zinc-500" />
              <LogIn className="h-3 w-3 text-zinc-500" />
              <span className="text-xs font-semibold text-zinc-500">Bejelentkezés</span>
            </Link>
          )}

          {/* Wallet Status - only show when authenticated */}
          {isAuthenticated &&
            (hasCreds ? (
              <div className="flex items-center gap-2 rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-3 py-1.5">
                <Wallet className="h-3 w-3 text-emerald-400" />
                <span className="text-xs font-semibold text-emerald-400">Wallet Connected</span>
              </div>
            ) : (
              <Link
                href="/settings"
                className="flex items-center gap-2 rounded-lg bg-amber-500/10 border border-amber-500/20 px-3 py-1.5 hover:bg-amber-500/15 transition-colors"
              >
                <Wallet className="h-3 w-3 text-amber-400" />
                <span className="text-xs font-semibold text-amber-400">Connect Wallet</span>
              </Link>
            ))}
        </div>

        {/* Right: Actions */}
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="flex h-9 w-9 items-center justify-center rounded-lg bg-white/3 hover:bg-white/5 transition-colors cursor-pointer"
          >
            <Settings className="h-4 w-4 text-zinc-400" />
          </button>

          {isAuthenticated ? (
            <button
              type="button"
              onClick={handleLogout}
              className="flex h-9 w-9 items-center justify-center rounded-lg bg-white/3 hover:bg-white/5 transition-colors cursor-pointer"
            >
              <LogOut className="h-4 w-4 text-zinc-400" />
            </button>
          ) : (
            <Link
              href="/login"
              className="flex h-9 w-9 items-center justify-center rounded-lg bg-white/3 hover:bg-white/5 transition-colors cursor-pointer"
            >
              <LogIn className="h-4 w-4 text-zinc-400" />
            </Link>
          )}
        </div>
      </div>
    </header>
  );
}
