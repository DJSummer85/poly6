"use client";

import { LogIn, LogOut, Settings, User, Wallet } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { TradingModeToggle } from "@/components/ui/trading-mode-toggle";
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
    <header className="border-b border-white/8 bg-zinc-950/85 backdrop-blur-xl">
      <div className="mx-auto flex max-w-7xl items-center justify-between gap-4 px-4 py-3 lg:px-6">
        <div className="min-w-0">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg border border-indigo-500/20 bg-indigo-500/10">
              <Settings className="h-4 w-4 text-indigo-400" />
            </div>
            <div className="min-w-0">
              <p className="whitespace-nowrap text-sm font-bold text-zinc-100">Command Center</p>
              <p className="hidden truncate text-xs text-zinc-500 lg:block">
                BTC Up/Down 5m trading cockpit
              </p>
            </div>
          </div>
        </div>

        <div className="hidden items-center gap-2 xl:flex">
          {isAuthenticated && user ? (
            <div className="flex items-center gap-2 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-3 py-1.5">
              <div className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
              <User className="h-3 w-3 text-green-400" />
              <span className="text-xs font-semibold text-green-400">@{user.username}</span>
            </div>
          ) : (
            <Link
              href="/login"
              className="flex items-center gap-2 rounded-full border border-white/10 bg-zinc-900/70 px-3 py-1.5 transition-colors hover:bg-zinc-800/80"
            >
              <div className="h-2 w-2 rounded-full bg-zinc-500" />
              <LogIn className="h-3 w-3 text-zinc-500" />
              <span className="text-xs font-semibold text-zinc-500">Bejelentkezés</span>
            </Link>
          )}

          {isAuthenticated &&
            (hasCreds ? (
              <div className="flex items-center gap-2 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-3 py-1.5">
                <Wallet className="h-3 w-3 text-emerald-400" />
                <span className="text-xs font-semibold text-emerald-400">Wallet Connected</span>
              </div>
            ) : (
              <Link
                href="/settings"
                className="flex items-center gap-2 rounded-full border border-amber-500/20 bg-amber-500/10 px-3 py-1.5 transition-colors hover:bg-amber-500/15"
              >
                <Wallet className="h-3 w-3 text-amber-400" />
                <span className="text-xs font-semibold text-amber-400">Connect Wallet</span>
              </Link>
            ))}
        </div>

        <div className="flex items-center gap-2">
          <div className="hidden sm:block">
            <TradingModeToggle />
          </div>

          <button
            type="button"
            className="flex h-9 w-9 cursor-pointer items-center justify-center rounded-lg border border-white/8 bg-white/3 transition-colors hover:bg-white/5"
          >
            <Settings className="h-4 w-4 text-zinc-400" />
          </button>

          {isAuthenticated ? (
            <button
              type="button"
              onClick={handleLogout}
              className="flex h-9 w-9 cursor-pointer items-center justify-center rounded-lg border border-white/8 bg-white/3 transition-colors hover:bg-white/5"
            >
              <LogOut className="h-4 w-4 text-zinc-400" />
            </button>
          ) : (
            <Link
              href="/login"
              className="flex h-9 w-9 cursor-pointer items-center justify-center rounded-lg border border-white/8 bg-white/3 transition-colors hover:bg-white/5"
            >
              <LogIn className="h-4 w-4 text-zinc-400" />
            </Link>
          )}
        </div>
      </div>
    </header>
  );
}
