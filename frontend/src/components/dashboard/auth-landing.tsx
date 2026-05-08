"use client";

import {
  Activity,
  ArrowRight,
  BarChart3,
  Bot,
  Clock3,
  Gauge,
  LineChart,
  Lock,
  LogIn,
  Radio,
  Settings,
  ShieldCheck,
  Target,
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
    <div className="grid min-h-[calc(100vh-7rem)] content-center gap-6 py-6">
      <section className="grid gap-6 lg:grid-cols-[minmax(0,1.1fr)_420px]">
        <div className="flex min-w-0 flex-col justify-center rounded-2xl border border-white/8 bg-white/3 p-6 shadow-2xl shadow-black/30 backdrop-blur-xl lg:p-8">
          <div className="mb-6 flex flex-wrap items-center gap-2">
            <StatusPill icon={Radio} label="SSE live feed" tone="green" />
            <StatusPill icon={Clock3} label="5m BTC markets" tone="amber" />
            <StatusPill icon={ShieldCheck} label="Encrypted keys" tone="indigo" />
          </div>

          <div className="max-w-2xl">
            <div className="mb-5 flex h-12 w-12 items-center justify-center rounded-xl border border-indigo-500/20 bg-indigo-500/10">
              <Zap className="h-6 w-6 text-indigo-400" />
            </div>
            <h1 className="text-3xl font-bold tracking-tight text-zinc-100 md:text-5xl">
              PolyTrade V2
            </h1>
            <p className="mt-4 max-w-xl text-base leading-7 text-zinc-400 md:text-lg">
              Polymarket BTC Up/Down bot dashboard kézi kontrollal, élő piaci állapottal és tiszta
              kockázati visszajelzésekkel.
            </p>
          </div>

          <div className="mt-8 flex flex-col gap-3 sm:flex-row">
            <LoginButton />
            <RegisterButton />
          </div>

          <div className="mt-8 grid gap-3 sm:grid-cols-3">
            <MetricTile label="Market cadence" value="5 min" detail="BTC windows" />
            <MetricTile label="Bot control" value="Manual" detail="explicit start/stop" />
            <MetricTile label="Execution" value="Demo/Live" detail="segmented mode" />
          </div>
        </div>

        <MarketPreviewCard />
      </section>

      <section className="grid gap-4 md:grid-cols-3">
        <FeatureCard
          icon={TrendingUp}
          title="Élő piac fókusz"
          description="BTC célár, delta, időzítő és odds egy áttekinthető cockpitben."
          tone="green"
        />
        <FeatureCard
          icon={Bot}
          title="Bot flotta kontroll"
          description="A bot kiválasztása, indítása és leállítása külön, explicit művelet."
          tone="blue"
        />
        <FeatureCard
          icon={ShieldCheck}
          title="Biztonságos hitelesítés"
          description="A Polymarket API kulcsok titkosítva kerülnek tárolásra."
          tone="violet"
        />
      </section>
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
      className="flex items-center justify-center gap-2 rounded-xl border border-white/10 px-6 py-3 text-sm font-semibold text-zinc-200 transition-colors hover:border-indigo-400/50 hover:bg-indigo-500/10 hover:text-indigo-200"
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
  tone,
}: {
  icon: React.ElementType;
  title: string;
  description: string;
  tone: "green" | "blue" | "violet";
}) {
  const toneClass = {
    blue: "border-blue-500/20 bg-blue-500/8 text-blue-300",
    green: "border-emerald-500/20 bg-emerald-500/8 text-emerald-300",
    violet: "border-violet-500/20 bg-violet-500/8 text-violet-300",
  }[tone];

  return (
    <div className="rounded-xl border border-white/8 bg-white/3 p-4 backdrop-blur-sm">
      <div className="flex items-start gap-3">
        <div
          className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border ${toneClass}`}
        >
          <Icon className="h-5 w-5" />
        </div>
        <div className="min-w-0">
          <h3 className="text-sm font-semibold text-zinc-200">{title}</h3>
          <p className="mt-1 text-xs leading-5 text-zinc-500">{description}</p>
        </div>
      </div>
    </div>
  );
}

function StatusPill({
  icon: Icon,
  label,
  tone,
}: {
  icon: React.ElementType;
  label: string;
  tone: "green" | "amber" | "indigo";
}) {
  const toneClass = {
    amber: "border-amber-500/20 bg-amber-500/10 text-amber-300",
    green: "border-emerald-500/20 bg-emerald-500/10 text-emerald-300",
    indigo: "border-indigo-500/20 bg-indigo-500/10 text-indigo-300",
  }[tone];

  return (
    <div className={`flex items-center gap-2 rounded-full border px-3 py-1.5 ${toneClass}`}>
      <Icon className="h-3.5 w-3.5" />
      <span className="text-xs font-semibold">{label}</span>
    </div>
  );
}

function MetricTile({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="rounded-xl border border-white/8 bg-zinc-950/50 p-3">
      <p className="text-[10px] font-bold uppercase tracking-wider text-zinc-500">{label}</p>
      <p className="mt-1 text-lg font-extrabold text-zinc-100">{value}</p>
      <p className="text-xs text-zinc-500">{detail}</p>
    </div>
  );
}

function MarketPreviewCard() {
  return (
    <div className="rounded-2xl border border-white/8 bg-zinc-950/80 p-5 shadow-2xl shadow-black/40 backdrop-blur-xl">
      <div className="mb-5 flex items-start justify-between gap-4">
        <div>
          <p className="text-xs font-bold uppercase tracking-wider text-zinc-500">Current market</p>
          <h2 className="mt-1 text-xl font-bold text-zinc-100">BTC closes above target?</h2>
        </div>
        <div className="rounded-full border border-emerald-500/20 bg-emerald-500/10 px-3 py-1 text-xs font-bold text-emerald-300">
          Live-ready
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <PreviewStat icon={LineChart} label="BTC" value="$102,430" tone="blue" />
        <PreviewStat icon={Target} label="Target" value="$102,400" tone="violet" />
        <PreviewStat icon={Activity} label="Delta" value="+30" tone="green" />
        <PreviewStat icon={Clock3} label="Remaining" value="03:12" tone="amber" />
      </div>

      <div className="mt-5 rounded-xl border border-white/8 bg-white/3 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Gauge className="h-4 w-4 text-emerald-300" />
            <span className="text-sm font-semibold text-zinc-200">Market bias</span>
          </div>
          <span className="font-mono text-sm font-bold text-emerald-300">YES 54%</span>
        </div>
        <div className="h-2 overflow-hidden rounded-full bg-zinc-800">
          <div className="h-full w-[54%] rounded-full bg-emerald-400" />
        </div>
        <div className="mt-3 flex items-center justify-between text-xs text-zinc-500">
          <span>NO 46%</span>
          <span>Volume monitor</span>
        </div>
      </div>

      <div className="mt-5 grid grid-cols-2 gap-3">
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/8 p-3">
          <BarChart3 className="mb-2 h-4 w-4 text-emerald-300" />
          <p className="text-xs font-semibold text-zinc-300">Clean signal layout</p>
        </div>
        <div className="rounded-xl border border-amber-500/20 bg-amber-500/8 p-3">
          <Lock className="mb-2 h-4 w-4 text-amber-300" />
          <p className="text-xs font-semibold text-zinc-300">Live mode guarded</p>
        </div>
      </div>
    </div>
  );
}

function PreviewStat({
  icon: Icon,
  label,
  value,
  tone,
}: {
  icon: React.ElementType;
  label: string;
  value: string;
  tone: "amber" | "blue" | "green" | "violet";
}) {
  const toneClass = {
    amber: "text-amber-300 bg-amber-500/10 border-amber-500/20",
    blue: "text-blue-300 bg-blue-500/10 border-blue-500/20",
    green: "text-emerald-300 bg-emerald-500/10 border-emerald-500/20",
    violet: "text-violet-300 bg-violet-500/10 border-violet-500/20",
  }[tone];

  return (
    <div className="rounded-xl border border-white/8 bg-white/3 p-3">
      <div className="mb-3 flex items-center justify-between gap-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-zinc-500">
          {label}
        </span>
        <div className={`flex h-7 w-7 items-center justify-center rounded-lg border ${toneClass}`}>
          <Icon className="h-3.5 w-3.5" />
        </div>
      </div>
      <p className="font-mono text-lg font-extrabold text-zinc-100">{value}</p>
    </div>
  );
}
