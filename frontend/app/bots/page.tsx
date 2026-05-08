"use client";

import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Activity, Bot as BotIcon, Loader2, Play, Plus, Square, Trash2, RotateCcw,
  Shield, Wallet, Search, ArrowUpDown, WifiOff, Trophy, AlertTriangle,
  TrendingUp, ScrollText, Filter, ChevronDown, BarChart3, History, Target
} from "lucide-react";
import { toast } from "sonner";
import { apiFetch } from "@/lib/utils";
import { AppShell } from "@/components/layout/app-shell";

// ---- Típusok ----
type BotStatus = "running" | "paused" | "error" | "stopped";
type SortKey = "pnl" | "winRate" | "trades" | "balance" | "name";

interface TradeResult {
  id: string;
  win: boolean;
  amount: number;
  time: string;
}

interface Bot {
  id: string;
  name: string;
  strategy_type: string;
  status: BotStatus;
  trading_mode: "paper" | "live";
  bet_size: number;
  stop_loss: number;
  take_profit: number;
  market_id: string;
  history?: TradeResult[];
  portfolio?: {
    balance: number;
    total_pnl: number;
    total_trades: number;
    winning_trades: number;
    losing_trades: number;
    win_rate: number;
    active_position?: any;
  };
}

const STRATEGY_COLORS: Record<string, string> = {
  momentum: "#818cf8",
  mean_reversion: "#34d399",
  last_seconds_scalp: "#f472b6",
  binance_signal: "#38bdf8",
  scalping: "#fb923c",
};

const STATUS_COLORS = {
  running: "bg-green-500",
  paused: "bg-amber-500",
  error: "bg-red-500",
  stopped: "bg-zinc-500",
};

export default function BotsPage() {
  const [bots, setBots] = useState<Bot[]>([]);
  const [logs, setLogs] = useState<{ id: string, time: string, msg: string, type: string }[]>([]);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<Date>(new Date());
  const [isSyncing, setIsSyncing] = useState(false);
  const [serverOnline, setServerOnline] = useState(true);
  const [mounted, setMounted] = useState(false);

  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | BotStatus>("all");
  const [sortKey, setSortKey] = useState<SortKey>("pnl");
  const [sortDir, setSortDir] = useState<"desc" | "asc">("desc");
  const [quickFilter, setQuickFilter] = useState<"none" | "best3" | "worst3">("none");
  const [expandedBot, setExpandedBot] = useState<string | null>(null);
  const [showFilters, setShowFilters] = useState(false);

  const prevBotsRef = useRef<Bot[]>([]);

  const addLog = useCallback((msg: string, type: string = "info") => {
    const newEntry = { id: Math.random().toString(), time: new Date().toLocaleTimeString(), msg, type };
    setLogs((prev) => [newEntry, ...prev].slice(0, 50));
  }, []);

  const loadBots = useCallback(async () => {
    setIsSyncing(true);
    try {
      const data = await apiFetch<Bot[]>("/bots");
      const withPortfolio = await Promise.all(
        data.map(async (bot) => {
          try {
            const p = await apiFetch<any>(`/bots/${bot.id}/portfolio`);
            return { ...bot, portfolio: p };
          } catch { return bot; }
        })
      );

      if (prevBotsRef.current.length > 0) {
        withPortfolio.forEach((newBot) => {
          const oldBot = prevBotsRef.current.find((b) => b.id === newBot.id);
          if (oldBot?.portfolio && newBot.portfolio) {
            if (newBot.portfolio.total_trades > oldBot.portfolio.total_trades) {
              const pnlDiff = newBot.portfolio.total_pnl - oldBot.portfolio.total_pnl;
              const isWin = pnlDiff >= 0;
              const newTrade = { id: Math.random().toString(), win: isWin, amount: Math.abs(pnlDiff), time: new Date().toLocaleTimeString() };
              newBot.history = [newTrade, ...(oldBot.history || [])].slice(0, 100);
              addLog(`${newBot.name}: ${isWin ? "NYERTES" : "VESZTES"} trade ($${Math.abs(pnlDiff).toFixed(2)})`, isWin ? "success" : "warn");
            } else {
              newBot.history = oldBot.history;
            }
          }
        });
      }

      prevBotsRef.current = withPortfolio;
      setBots(withPortfolio);
      setLastSync(new Date());
      setServerOnline(true);
    } catch (err) {
      setServerOnline(false);
    } finally {
      setIsSyncing(false);
    }
  }, [addLog]);

  useEffect(() => {
    setMounted(true);
    loadBots();
    const interval = setInterval(loadBots, 15000);
    return () => clearInterval(interval);
  }, [loadBots]);

  const handleStart = async (id: string, name: string) => {
    setActionLoading(id);
    try {
      await apiFetch(`/bots/${id}/start`, { method: "POST" });
      toast.success(`${name} elindítva`);
      await loadBots();
    } catch (err: any) { toast.error(err.message); }
    finally { setActionLoading(null); }
  };

  const handleStop = async (id: string, name: string) => {
    setActionLoading(id);
    try {
      await apiFetch(`/bots/${id}/stop`, { method: "POST" });
      toast.success(`${name} leállítva`);
      await loadBots();
    } catch (err: any) { toast.error(err.message); }
    finally { setActionLoading(null); }
  };

  const handleReset = async (id: string, name: string) => {
    if (!confirm(`Reset stat: ${name}?`)) return;
    setActionLoading(id);
    try {
      await apiFetch(`/bots/${id}/reset`, { method: "POST" });
      toast.success("Bot nullázva");
      setBots(prev => prev.map(b => b.id === id ? { ...b, history: [] } : b));
      await loadBots();
    } catch { toast.error("Reset hiba"); }
    finally { setActionLoading(null); }
  };

  const handleResetAll = async () => {
    if (!confirm("Összes bot nullázása?")) return;
    try {
      await Promise.all(bots.map(b => apiFetch(`/bots/${b.id}/reset`, { method: "POST" })));
      toast.success("Minden bot nullázva!");
      loadBots();
    } catch { toast.error("Hiba"); }
  };

  const handleBulkAction = async (action: "start" | "stop") => {
    const targets = bots.filter(b => action === "start" ? b.status !== "running" : b.status === "running");
    if (targets.length === 0) return;
    toast.promise(Promise.all(targets.map(b => apiFetch(`/bots/${b.id}/${action}`, { method: "POST" }))), {
      loading: "Művelet...",
      success: "Kész!",
      error: "Hiba"
    });
    setTimeout(loadBots, 2000);
  };

  const filteredBots = useMemo(() => {
    let list = [...bots];
    if (search) list = list.filter(b => b.name.toLowerCase().includes(search.toLowerCase()));
    if (statusFilter !== "all") list = list.filter(b => b.status === statusFilter);
    list.sort((a, b) => {
      let valA: any = sortKey === "pnl" ? a.portfolio?.total_pnl || 0 : (a as any)[sortKey] || 0;
      let valB: any = sortKey === "pnl" ? b.portfolio?.total_pnl || 0 : (b as any)[sortKey] || 0;
      if (sortKey === "name") return sortDir === "desc" ? b.name.localeCompare(a.name) : a.name.localeCompare(b.name);
      return sortDir === "desc" ? valB - valA : valA - valB;
    });
    if (quickFilter === "best3") return [...list].sort((a, b) => (b.portfolio?.total_pnl || 0) - (a.portfolio?.total_pnl || 0)).slice(0, 3);
    if (quickFilter === "worst3") return [...list].sort((a, b) => (a.portfolio?.total_pnl || 0) - (b.portfolio?.total_pnl || 0)).slice(0, 3);
    return list;
  }, [bots, search, statusFilter, sortKey, sortDir, quickFilter]);

  const totalStats = {
    active: bots.filter(b => b.status === "running").length,
    pnl: bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0),
    balance: bots.reduce((a, b) => a + (b.portfolio?.balance || 0), 0),
    trades: bots.reduce((a, b) => a + (b.portfolio?.total_trades || 0), 0),
    wins: bots.reduce((a, b) => a + (b.portfolio?.winning_trades || 0), 0),
    losses: bots.reduce((a, b) => a + (b.portfolio?.losing_trades || 0), 0),
  };

  if (!mounted) return null;

  return (
    <AppShell>
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-indigo-500/20 bg-indigo-500/10"><BotIcon className="h-6 w-6 text-indigo-400" /></div>
            <div><h1 className="text-2xl font-bold text-white">Bot Fleet Manager</h1><p className="text-sm text-zinc-500">{bots.length} bot · {totalStats.active} aktív</p></div>
          </div>
          <button className="flex items-center gap-2 rounded-lg bg-indigo-500 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-600"><Plus size={16} />Új bot</button>
        </div>

        <div className="grid grid-cols-2 gap-4 lg:grid-cols-6">
          <StatCard label="AKTÍV BOTOK" value={totalStats.active} icon={<Activity size={16} />} color="green" />
          <StatCard label="ÖSSZES PNL" value={`$${totalStats.pnl.toFixed(2)}`} icon={<TrendingUp size={16} />} color={totalStats.pnl >= 0 ? "green" : "red"} />
          <StatCard label="ÖSSZES TRADE" value={totalStats.trades} icon={<BarChart3 size={16} />} color="blue" />
          <StatCard label="EGYENLEG" value={`$${totalStats.balance.toFixed(2)}`} icon={<Wallet size={16} />} color="indigo" />
          <StatCard label="WIN RATE" value={`${((totalStats.wins / totalStats.trades) * 100 || 0).toFixed(1)}%`} icon={<Trophy size={16} />} color="amber" />
          <StatCard label="W / L" value={`${totalStats.wins} / ${totalStats.losses}`} icon={<Shield size={16} />} color="violet" />
        </div>

        <div className="flex flex-wrap items-center gap-3 rounded-xl border border-white/5 bg-zinc-900/50 p-4">
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-500" />
            <input type="text" placeholder="Bot keresése..." value={search} onChange={(e) => setSearch(e.target.value)} className="w-full rounded-lg border border-white/10 bg-zinc-800/50 py-2.5 pl-10 pr-4 text-sm text-white outline-none focus:border-indigo-500/50" />
          </div>
          <div className="flex rounded-lg border border-white/10 bg-zinc-800/30 p-1">
            {["all", "running", "stopped", "error"].map((f) => (
              <button key={f} onClick={() => setStatusFilter(f as any)} className={`rounded-md px-3 py-1.5 text-xs font-medium transition-all ${statusFilter === f ? "bg-indigo-500/20 text-indigo-400" : "text-zinc-500 hover:text-zinc-300"}`}>
                {f === "all" ? "Összes" : f === "running" ? `● Aktív` : f === "stopped" ? "Leállítva" : "Hiba"}
              </button>
            ))}
          </div>
          <select value={sortKey} onChange={(e) => setSortKey(e.target.value as SortKey)} className="rounded-lg border border-white/10 bg-zinc-800/50 px-3 py-2.5 text-sm text-white outline-none"><option value="pnl">Profit</option><option value="winRate">Win Rate</option><option value="balance">Egyenleg</option><option value="name">Név</option></select>
          <button onClick={() => setSortDir(d => d === "asc" ? "desc" : "asc")} className="flex h-10 w-10 items-center justify-center rounded-lg border border-white/10 bg-zinc-800/50 text-zinc-400 hover:text-white"><ArrowUpDown size={16} /></button>
          <button onClick={() => setShowFilters(!showFilters)} className={`flex h-10 items-center gap-2 rounded-lg border px-3 text-sm transition-all ${showFilters ? "border-indigo-500/30 bg-indigo-500/10 text-indigo-400" : "border-white/10 bg-zinc-800/50 text-zinc-400 hover:text-white"}`}><Filter size={16} />Gyorsszűrők <ChevronDown size={12} className={showFilters ? "rotate-180" : ""} /></button>
        </div>

        <AnimatePresence>
          {showFilters && (
            <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="flex flex-wrap items-center gap-2 p-4 bg-zinc-900/30 border border-white/5 rounded-xl overflow-hidden">
              <button onClick={() => setQuickFilter(quickFilter === "best3" ? "none" : "best3")} className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold border transition-all ${quickFilter === "best3" ? "bg-amber-500/20 border-amber-500/40 text-amber-400" : "bg-zinc-800/50 border-white/5 text-zinc-500"}`}><Trophy size={14} />Top 3 Legjobb</button>
              <button onClick={() => setQuickFilter(quickFilter === "worst3" ? "none" : "worst3")} className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold border transition-all ${quickFilter === "worst3" ? "bg-red-500/20 border-red-500/40 text-red-400" : "bg-zinc-800/50 border-white/5 text-zinc-500"}`}><AlertTriangle size={14} />Top 3 Legrosszabb</button>
              <div className="w-px h-6 bg-white/10 mx-2" />
              <button onClick={() => handleBulkAction("start")} className="px-3 py-1.5 rounded-md bg-green-500/10 text-green-400 text-xs font-bold border border-green-500/20">▶ Indít mind</button>
              <button onClick={() => handleBulkAction("stop")} className="px-3 py-1.5 rounded-md bg-amber-500/10 text-amber-400 text-xs font-bold border border-amber-500/20">■ Megállít mind</button>
              <button onClick={handleResetAll} className="px-3 py-1.5 rounded-md bg-indigo-500/10 text-indigo-400 text-xs font-bold border border-indigo-500/20">↺ Összes nullázása</button>
            </motion.div>
          )}
        </AnimatePresence>

        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          {filteredBots.map((bot) => (
            <BotCard key={bot.id} bot={bot} isExpanded={expandedBot === bot.id} isLoading={actionLoading === bot.id} onToggle={() => setExpandedBot(expandedBot === bot.id ? null : bot.id)} onStart={() => handleStart(bot.id, bot.name)} onStop={() => handleStop(bot.id, bot.name)} onReset={() => handleReset(bot.id, bot.name)} onDelete={() => { if (confirm("Törlés?")) apiFetch(`/bots/${bot.id}`, { method: "DELETE" }).then(loadBots); }} />
          ))}
        </div>

        <div className="rounded-xl border border-white/5 bg-zinc-900/50 p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2 text-indigo-400"><ScrollText size={18} /><h3 className="text-sm font-bold uppercase tracking-wider">Eseménynapló</h3></div>
            <div className="text-[10px] text-zinc-500 flex gap-4">
              <span className="flex items-center gap-1.5 text-green-500"><span className="h-1.5 w-1.5 rounded-full bg-green-500 animate-pulse" /> ONLINE</span>
              <span>Frissítve: {lastSync.toLocaleTimeString()}</span>
              {isSyncing && <Loader2 size={12} className="animate-spin" />}
            </div>
          </div>
          <div className="max-h-32 overflow-y-auto space-y-1.5 pr-2 scrollbar-thin scrollbar-thumb-zinc-800">
            {logs.map(log => (
              <div key={log.id} className="flex justify-between text-[11px] bg-zinc-800/30 px-3 py-1.5 rounded border border-white/[0.02]">
                <span className={log.type === "success" ? "text-green-400" : log.type === "warn" ? "text-amber-400" : log.type === "error" ? "text-red-400" : "text-zinc-400"}>{log.msg}</span>
                <span className="text-zinc-600 font-mono">{log.time}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </AppShell>
  );
}

function StatCard({ label, value, icon, color }: any) {
  const colors: any = { green: "text-green-400 bg-green-500/10 border-green-500/20", red: "text-red-400 bg-red-500/10 border-red-500/20", blue: "text-blue-400 bg-blue-500/10 border-blue-500/20", indigo: "text-indigo-400 bg-indigo-500/10 border-indigo-500/20", amber: "text-amber-400 bg-amber-500/10 border-amber-500/20", violet: "text-violet-400 bg-violet-500/10 border-violet-500/20" };
  return (
    <div className={`rounded-xl border p-4 ${colors[color]}`}>
      <div className="mb-1 flex items-center gap-2 opacity-60"><div className="p-1 rounded-md bg-black/20">{icon}</div><span className="text-[10px] font-bold uppercase">{label}</span></div>
      <p className="text-xl font-bold">{value}</p>
    </div>
  );
}

function BotCard({ bot, isExpanded, isLoading, onToggle, onStart, onStop, onReset, onDelete }: any) {
  const pnl = bot.portfolio?.total_pnl || 0;
  const balance = bot.portfolio?.balance || 0;
  const strategyColor = STRATEGY_COLORS[bot.strategy_type] || "#818cf8";
  const isRunning = bot.status === "running";

  // LOGIKA: Pozícióban van, ha az active_position létezik és nem üres
  const pos = bot.portfolio?.active_position;
  const isInPosition = pos && (typeof pos === 'object' ? Object.keys(pos).length > 0 : true);

  return (
    <motion.div
      layout
      className={`rounded-xl border transition-all duration-500 relative overflow-hidden ${isInPosition
          ? "bg-green-600/15 border-green-500/60 shadow-[0_0_25px_rgba(34,197,94,0.15)]"
          : isRunning ? "bg-green-500/[0.03] border-white/10" : "bg-zinc-900/50 border-white/5"
        }`}
    >
      {/* POZÍCIÓ JELZŐ BADGE */}
      {isInPosition && (
        <div className="absolute top-0 right-0 z-10">
          <div className="bg-green-500 text-black text-[9px] font-black px-2.5 py-1 rounded-bl-lg uppercase tracking-tighter shadow-lg">
            Pozícióban
          </div>
        </div>
      )}

      <div className="p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-3">
            <div className={`h-2.5 w-2.5 rounded-full ${STATUS_COLORS[bot.status]} ${isRunning ? "animate-pulse shadow-[0_0_8px_rgba(34,197,94,1)]" : ""}`} />
            <div>
              <h3 className="text-sm font-bold text-white">{bot.name}</h3>
              <span className="text-[9px] font-black uppercase px-1.5 py-0.5 rounded mt-1 inline-block" style={{ color: strategyColor, background: `${strategyColor}15` }}>{bot.strategy_type}</span>
            </div>
          </div>
          <div className="text-right">
            <p className={`text-base font-black leading-none ${pnl >= 0 ? "text-green-400" : "text-red-400"}`}>{pnl >= 0 ? "+" : ""}${pnl.toFixed(2)}</p>
            <p className="text-[10px] text-zinc-500 mt-1 font-mono">${balance.toFixed(2)}</p>
          </div>
        </div>

        <div className="flex items-center justify-between pt-3 border-t border-white/[0.05]">
          <div className="flex gap-4">
            <div className="text-center"><p className="text-[8px] text-zinc-500 uppercase font-bold">Win Rate</p><p className="text-xs font-bold text-zinc-300">{(bot.portfolio?.win_rate || 0).toFixed(1)}%</p></div>
            <div className="text-center"><p className="text-[8px] text-zinc-500 uppercase font-bold">Trades</p><p className="text-xs font-bold text-zinc-300">{bot.portfolio?.total_trades || 0}</p></div>
          </div>
          <button onClick={onToggle} className="text-zinc-600 hover:text-white transition-colors p-1"><ChevronDown size={18} className={isExpanded ? "rotate-180" : ""} /></button>
        </div>
      </div>

      <AnimatePresence>
        {isExpanded && (
          <motion.div initial={{ height: 0 }} animate={{ height: "auto" }} exit={{ height: 0 }} className="overflow-hidden border-t border-white/5 bg-black/30">
            <div className="p-4 space-y-4">

              {isInPosition && (
                <div className="bg-green-500/10 border border-green-500/20 rounded-lg p-2 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Target size={14} className="text-green-400 animate-spin-slow" />
                    <span className="text-[10px] font-bold text-green-400 uppercase tracking-widest">Aktív kötés</span>
                  </div>
                  <TrendingUp size={14} className="text-green-400 animate-pulse" />
                </div>
              )}

              <div className="grid grid-cols-3 gap-2">
                <div className="bg-zinc-800/50 p-2 rounded-lg text-center border border-white/[0.02]"><p className="text-[8px] text-zinc-500 uppercase mb-1">Tét</p><p className="text-xs font-bold text-white">${bot.bet_size}</p></div>
                <div className="bg-red-500/5 p-2 rounded-lg text-center border border-red-500/10"><p className="text-[8px] text-red-500/50 uppercase mb-1">SL</p><p className="text-xs font-bold text-red-500">-{(bot.stop_loss * 100).toFixed(0)}%</p></div>
                <div className="bg-green-500/5 p-2 rounded-lg text-center border border-green-500/10"><p className="text-[8px] text-green-500/50 uppercase mb-1">TP</p><p className="text-xs font-bold text-green-500">+{(bot.take_profit * 100).toFixed(0)}%</p></div>
              </div>

              <div className="max-h-32 overflow-y-auto space-y-1 pr-1 scrollbar-thin scrollbar-thumb-zinc-800">
                <p className="text-[9px] font-bold text-zinc-600 mb-2 uppercase flex items-center gap-1.5"><History size={10} /> Utóbbi kötések</p>
                {bot.history?.map((t: any) => (
                  <div key={t.id} className="flex justify-between items-center text-[10px] bg-zinc-800/40 px-2 py-1.5 rounded">
                    <span className={t.win ? "text-green-400 font-bold" : "text-red-400 font-bold"}>{t.win ? "✅ NYERT" : "❌ VESZTETT"}</span>
                    <span className="font-mono text-white opacity-80">${t.amount.toFixed(2)}</span>
                  </div>
                ))}
              </div>

              <div className="flex gap-2">
                <button onClick={isRunning ? onStop : onStart} disabled={isLoading} className={`flex-1 flex items-center justify-center gap-2 py-2 rounded-lg text-xs font-bold transition-all ${isRunning ? "bg-amber-500/10 text-amber-500 border border-amber-500/20" : "bg-green-500/10 text-green-500 border border-green-500/20"}`}>
                  {isLoading ? <Loader2 size={14} className="animate-spin" /> : isRunning ? <><Square size={14} /> STOP</> : <><Play size={14} /> START</>}
                </button>
                <button onClick={onReset} className="p-2 rounded-lg bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 hover:bg-indigo-500/20"><RotateCcw size={16} /></button>
                <button onClick={onDelete} className="p-2 rounded-lg bg-red-500/10 text-red-500 border border-red-500/20 hover:bg-red-500/20"><Trash2 size={16} /></button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}