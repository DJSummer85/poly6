"use client";

import {
  ArrowRight,
  Lock,
  LogIn,
  Settings,
  ShieldCheck,
  TrendingUp,
  Wallet,
  Zap,
} from "lucide-react";
import Link from "next/link";
import { useSettings } from "@/hooks";
import { useAppStore } from "@/store";

export function AuthLanding() {
  const { isAuthenticated, user } = useAppStore();

  if (isAuthenticated && user) {
    return <LoggedInContent />;
  }

  return <NotLoggedInContent />;
}

function NotLoggedInContent() {
  return (
    <div className="flex flex-col items-center justify-center gap-8 py-12">
      {/* Hero */}
      <div className="flex flex-col items-center text-center max-w-xl">
        <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-indigo-500/15 mb-6">
          <Zap className="h-8 w-8 text-indigo-400" />
        </div>
        <h1 className="text-3xl font-bold text-zinc-100 mb-2">
          Üdv a <span className="text-indigo-400">PolyTrade</span> V2-ben
        </h1>
        <p className="text-zinc-400 text-lg">
          Automatizált trading botok Polymarket BTC Up/Down piacokhoz
        </p>
      </div>

      {/* Feature cards */}
      <div className="grid gap-4 w-full max-w-lg">
        <FeatureCard
          icon={TrendingUp}
          title="Valós idejű BTC kereskedés"
          description="5 perces piacok, Kelly-kritérium alapú pozíciókezelés"
          color="text-green-400"
          bg="bg-green-500/10"
          border="border-green-500/20"
        />
        <FeatureCard
          icon={ShieldCheck}
          title="Biztonságos, encrypted hitelesítés"
          description="API kulcsok AES-256-GCM titkosítással az adatbázisban"
          color="text-violet-400"
          bg="bg-violet-500/10"
          border="border-violet-500/20"
        />
      </div>

      {/* Auth CTA */}
      <div className="flex gap-3 mt-2">
        <LoginButton />
        <RegisterButton />
      </div>
    </div>
  );
}

function LoggedInContent() {
  const { user } = useAppStore();
  const { data: settings, isLoading } = useSettings();
  const hasCreds = settings?.has_credentials ?? false;

  if (isLoading) {
    return (
      <div className="flex flex-col items-center justify-center py-12 gap-4">
        <div className="h-12 w-12 rounded-xl border-2 border-indigo-500/30 border-t-indigo-500 animate-spin" />
        <p className="text-zinc-400 text-sm">Fiók ellenőrzése...</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center justify-center gap-8 py-12">
      {/* Welcome */}
      <div className="flex flex-col items-center text-center max-w-lg">
        <h1 className="text-3xl font-bold text-zinc-100 mb-2">
          Hello, <span className="text-indigo-400">{user?.username ?? "Felhasználó"}</span>!
        </h1>
        <p className="text-zinc-400 text-lg">
          {hasCreds
            ? "A fiókod készen áll a tradingre"
            : "A fiókod létrejött, de még nincs csatlakoztatva Polymarket"}
        </p>
      </div>

      {hasCreds ? <ConnectedStatus /> : <NotConnectedStatus />}
    </div>
  );
}

function NotConnectedStatus() {
  return (
    <div className="flex flex-col items-center gap-6 w-full max-w-md">
      {/* Not connected card */}
      <div className="glass-card w-full p-6 flex flex-col items-center text-center gap-4">
        <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-amber-500/15">
          <Lock className="h-6 w-6 text-amber-400" />
        </div>
        <div>
          <h3 className="text-lg font-semibold text-zinc-200 mb-1">Nincs csatlakoztatva</h3>
          <p className="text-zinc-400 text-sm max-w-xs">
            A trading botok használatához add hozzá a Polymarket API kulcsaidat a Beállításokban.
          </p>
        </div>
      </div>

      <Link href="/settings" className="btn-primary flex items-center gap-2 px-6">
        <Settings className="h-4 w-4" />
        <span>Beállítások megnyitása</span>
        <ArrowRight className="h-4 w-4" />
      </Link>

      <p className="text-xs text-zinc-500 text-center max-w-sm">
        Az API kulcsokat a{" "}
        <a
          href="https://polymarket.com"
          target="_blank"
          rel="noopener noreferrer"
          className="text-indigo-400 underline"
        >
          Polymarket
        </a>{" "}
        fiókodban generálhatod.
      </p>
    </div>
  );
}

function ConnectedStatus() {
  return (
    <div className="flex flex-col items-center gap-4 w-full max-w-md">
      <div className="glass-card w-full p-6 flex flex-col items-center text-center gap-3">
        <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-green-500/15">
          <Wallet className="h-6 w-6 text-green-400" />
        </div>
        <div>
          <h3 className="text-lg font-semibold text-zinc-200 mb-1">Polymarket kapcsolódva</h3>
          <p className="text-zinc-400 text-sm">
            A trading botok készen állnak. Indíthatsz egy bot-ot vagy köthetsz éles ügyletet is.
          </p>
        </div>
      </div>

      <Link href="/bots" className="btn-primary flex items-center gap-2 px-6">
        <TrendingUp className="h-4 w-4" />
        <span>Bot indítása</span>
        <ArrowRight className="h-4 w-4" />
      </Link>
    </div>
  );
}

// --- Shared components ---

function LoginButton() {
  return (
    <Link href="/login" className="btn-primary flex items-center gap-2 px-6">
      <LogIn className="h-4 w-4" />
      <span>Bejelentkezés</span>
    </Link>
  );
}

function RegisterButton() {
  return (
    <Link
      href="/register"
      className="flex items-center gap-2 px-6 py-2.5 rounded-xl border border-indigo-500/30 text-indigo-400 hover:bg-indigo-500/10 transition-colors"
    >
      <span>Regisztráció</span>
      <ArrowRight className="h-4 w-4" />
    </Link>
  );
}

function FeatureCard({
  icon: Icon,
  title,
  description,
  color,
  bg,
  border,
}: {
  icon: React.ElementType;
  title: string;
  description: string;
  color: string;
  bg: string;
  border: string;
}) {
  return (
    <div className={`rounded-xl border p-4 ${bg} ${border} backdrop-blur-sm`}>
      <div className="flex items-center gap-3">
        <div className={`flex h-10 w-10 items-center justify-center rounded-lg ${bg}`}>
          <Icon className={`h-5 w-5 ${color}`} />
        </div>
        <div>
          <h3 className="text-sm font-semibold text-zinc-200">{title}</h3>
          <p className="text-xs text-zinc-400">{description}</p>
        </div>
      </div>
    </div>
  );
}
