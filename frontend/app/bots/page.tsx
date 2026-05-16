'use client'

import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Activity, Bot as BotIcon, Loader2, Play, Plus, Square, Trash2, RotateCcw,
  Shield, Target, Wallet, Search, ArrowUpDown, Wifi, WifiOff, Trophy, AlertTriangle,
  X, TrendingUp, ScrollText, Clock, History, Zap, ChevronDown, Settings,
  Download, AlertCircle, Brain
} from "lucide-react"
import { toast } from "sonner"
import { apiFetch } from "@/lib/utils"
import { useAppStore } from "@/store"
import { useSSE, useBtcPrice } from "@/hooks"
import { BotThoughts } from "@/components/dashboard/bot-thoughts"

// ---- Típusok ----
type BotStatus = 'running' | 'paused' | 'error' | 'stopped'
type SortKey = 'balance' | 'pnl' | 'wins' | 'losses' | 'trades'

interface TradeResult { id: string; win: boolean; amount: number; time: string; }

interface Bot {
  id: string; name: string; strategy_type: string; status: BotStatus;
  trading_mode: 'paper' | 'live'; bet_size: number; stop_loss: number;
  take_profit: number; market_id: string; history?: TradeResult[];
  // Számlálók indítási időpontjai
  runSince?: number;      // timestamp (ms), mióta fut folyamatosan
  posSince?: number;      // timestamp (ms), mióta van pozícióban
  portfolio?: {
    balance: number; initial_balance: number; total_pnl: number;
    total_trades: number; winning_trades: number; losing_trades: number;
    win_rate: number; open_positions: number;
    unrealized_pnl: number;       // backend küldi, pozíció jelzéshez
    total_position_value: number; // backend küldi, pozíció jelzéshez
  }
}

const STRATEGY_COLORS: Record<string, string> = {
  momentum: '#818cf8', mean_reversion: '#34d399', last_seconds_scalp: '#f472b6',
  binance_signal: '#38bdf8', scalping: '#fb923c'
}

// ---- Időformázó segédfüggvény ----
function formatElapsed(ms: number): string {
  const totalSec = Math.floor(ms / 1000)
  const h = Math.floor(totalSec / 3600)
  const m = Math.floor((totalSec % 3600) / 60)
  const s = totalSec % 60
  if (h > 0) return `${h}ó ${m}p ${s}mp`
  if (m > 0) return `${m}p ${s}mp`
  return `${s}mp`
}

// ---- Élő időszámláló hook ----
function useElapsedTimer(startTs: number | undefined): string {
  const [elapsed, setElapsed] = useState<string>('—')

  useEffect(() => {
    if (!startTs) { setElapsed('—'); return }
    const update = () => setElapsed(formatElapsed(Date.now() - startTs))
    update()
    const id = setInterval(update, 1000)
    return () => clearInterval(id)
  }, [startTs])

  return elapsed
}

export default function BotsPage() {
  const { tradingMode, setTradingMode, btcPrice, timeRemaining, yesPrice } = useAppStore()
  useSSE()
  useBtcPrice()
  const [bots, setBots] = useState<Bot[]>([])
  const [logs, setLogs] = useState<any[]>([])
  const [actionLoading, setActionLoading] = useState<string | null>(null)
  const [lastSync, setLastSync] = useState<Date>(new Date())
  const [isSyncing, setIsSyncing] = useState(false)
  const [serverOnline, setServerOnline] = useState(true)
  const [mounted, setMounted] = useState(false)
  const prevBotsRef = useRef<Bot[]>([])

  // UI State
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | BotStatus>('all')
  const [sortKey, setSortKey] = useState<SortKey>('pnl')
  const [sortDir, setSortDir] = useState<'desc' | 'asc'>('desc')
  const [quickFilter, setQuickFilter] = useState<'none' | 'best3' | 'worst3'>('none')
  const [editingBot, setEditingBot] = useState<Bot | null>(null)
  const [isUpdating, setIsUpdating] = useState(false)
  const [showThoughts, setShowThoughts] = useState(false)

  const addLog = useCallback((msg: string, type: string = 'info') => {
    const newEntry = { id: Math.random().toString(), time: new Date().toLocaleTimeString(), msg, type }
    setLogs(prev => [newEntry, ...prev].slice(0, 20))
  }, [])

  // POZÍCIÓ ELLENŐRZÉSE
  // Háromféle jelzés alapján: open_positions, unrealized_pnl, total_position_value
  // Demo módban a Polymarket API nem elérhető, ezért az unrealized_pnl és
  // total_position_value mezőket is figyelembe vesszük
  const checkPosition = (bot: Bot) => {
    if (!bot.portfolio) return false
    if ((bot.portfolio.open_positions ?? 0) > 0) return true
    if (Math.abs(bot.portfolio.unrealized_pnl ?? 0) > 0.001) return true
    if ((bot.portfolio.total_position_value ?? 0) > 0.001) return true
    return false
  }

  const loadBots = useCallback(async () => {
    setIsSyncing(true)
    try {
      const data = await apiFetch<Bot[]>("/bots")
      const withPortfolio = await Promise.all(
        data.map(async (bot) => {
          try {
            const p = await apiFetch<any>(`/bots/${bot.id}/portfolio`)
            return { ...bot, portfolio: p }
          } catch { return bot }
        })
      )

      if (prevBotsRef.current.length > 0) {
        withPortfolio.forEach((newBot) => {
          const oldBot = prevBotsRef.current.find(b => b.id === newBot.id)
          if (oldBot?.portfolio && newBot.portfolio) {
            if (newBot.portfolio.total_trades > oldBot.portfolio.total_trades) {
              const diff = newBot.portfolio.total_pnl - oldBot.portfolio.total_pnl
              const isWin = diff >= 0
              const newTrade = { id: Math.random().toString(), win: isWin, amount: Math.abs(diff), time: new Date().toLocaleTimeString() }
              newBot.history = [newTrade, ...(oldBot.history || [])].slice(0, 100)
              addLog(`${newBot.name}: ${isWin ? 'NYERTES' : 'VESZTES'} trade ($${Math.abs(diff).toFixed(2)})`, isWin ? 'success' : 'warn')
            } else { newBot.history = oldBot.history }
          }

          // ---- FUTÁSI IDŐ SZÁMLÁLÓ logika ----
          if (oldBot) {
            if (oldBot.status !== 'running' && newBot.status === 'running') {
              newBot.runSince = Date.now()
            } else if (newBot.status === 'running') {
              newBot.runSince = oldBot.runSince
            } else {
              newBot.runSince = undefined
            }

            // ---- POZÍCIÓ IDŐ SZÁMLÁLÓ logika ----
            const wasInPos = checkPosition(oldBot as Bot)
            const isNowInPos = checkPosition(newBot as Bot)
            if (!wasInPos && isNowInPos) {
              newBot.posSince = Date.now()
            } else if (isNowInPos) {
              newBot.posSince = oldBot.posSince
            } else {
              newBot.posSince = undefined
            }
          } else {
            if (newBot.status === 'running') newBot.runSince = Date.now()
            if (checkPosition(newBot as Bot)) newBot.posSince = Date.now()
          }
        })
      } else {
        withPortfolio.forEach((bot) => {
          if (bot.status === 'running') bot.runSince = Date.now()
          if (checkPosition(bot as Bot)) bot.posSince = Date.now()
        })
      }

      prevBotsRef.current = withPortfolio
      setBots(withPortfolio)
      setLastSync(new Date())
      setServerOnline(true)
    } catch (err) {
      setServerOnline(false)
    } finally {
      setIsSyncing(false)
    }
  }, [addLog])

  useEffect(() => {
    setMounted(true)
    loadBots()
    const interval = setInterval(loadBots, 15000)
    return () => clearInterval(interval)
  }, [loadBots])

  const handleBulk = async (action: string) => {
    try {
      await apiFetch(`/bots/${action}`, { method: "POST" })
      toast.success(`Művelet sikeres: ${action}`)
      loadBots()
    } catch (e: any) { toast.error(e.message) }
  }

  const handleBotAction = async (id: string, action: string) => {
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/${action}`, { method: "POST" })
      loadBots()
    } catch (e: any) {
      if (!e.message.includes("409")) toast.error(e.message)
    } finally { setActionLoading(null) }
  }

  const handleUpdateBot = async (id: string, updates: Partial<Bot>) => {
    setIsUpdating(true)
    try {
      await apiFetch(`/bots/${id}`, {
        method: "PUT",
        body: JSON.stringify(updates)
      })
      toast.success("Bot beállítások mentve")
      setEditingBot(null)
      loadBots()
    } catch (e: any) {
      toast.error(e.message)
    } finally {
      setIsUpdating(false)
    }
  }

  const handleModeChange = async (mode: "demo" | "live") => {
    setTradingMode(mode)
    try {
      const trading_mode = mode === "live" ? "live" : "paper"
      await apiFetch("/bots/set-mode", { method: "POST", body: JSON.stringify({ trading_mode }) })
      toast.success(`Mód átváltva: ${mode.toUpperCase()}`)
      loadBots()
    } catch (e: any) { toast.error("Módváltás sikertelen") }
  }

  const botsInPosition = useMemo(() => bots.filter(checkPosition), [bots]);

  const filteredBots = useMemo(() => {
    let list = [...bots]
    if (search) list = list.filter(b => b.name.toLowerCase().includes(search.toLowerCase()))
    if (statusFilter !== 'all') list = list.filter(b => b.status === statusFilter)

    list.sort((a, b) => {
      let valA = 0;
      let valB = 0;

      switch (sortKey) {
        case 'balance': valA = a.portfolio?.balance || 0; valB = b.portfolio?.balance || 0; break;
        case 'pnl': valA = a.portfolio?.total_pnl || 0; valB = b.portfolio?.total_pnl || 0; break;
        case 'wins': valA = a.portfolio?.winning_trades || 0; valB = b.portfolio?.winning_trades || 0; break;
        case 'losses': valA = a.portfolio?.losing_trades || 0; valB = b.portfolio?.losing_trades || 0; break;
        case 'trades': valA = a.portfolio?.total_trades || 0; valB = b.portfolio?.total_trades || 0; break;
      }

      return sortDir === 'desc' ? valB - valA : valA - valB
    })

    if (quickFilter === 'best3') return [...list].sort((a, b) => (b.portfolio?.total_pnl || 0) - (a.portfolio?.total_pnl || 0)).slice(0, 3)
    if (quickFilter === 'worst3') return [...list].sort((a, b) => (a.portfolio?.total_pnl || 0) - (b.portfolio?.total_pnl || 0)).slice(0, 3)
    return list
  }, [bots, search, statusFilter, sortKey, sortDir, quickFilter])

  if (!mounted) return null

  return (
    <div style={{ padding: '24px', background: '#0b0b14', minHeight: '100vh', color: '#e2e8f0', fontFamily: 'system-ui, sans-serif' }}>

      {/* 1. FEJLÉC */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '25px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '15px' }}>
          <div style={{ background: '#1a1a2e', padding: '10px', borderRadius: '12px', border: '1px solid #252535' }}><BotIcon size={24} color="#6366f1" /></div>
          <div><h1 style={{ fontSize: '22px', fontWeight: 700, margin: 0 }}>Bot Fleet Manager</h1><p style={{ color: '#4b5563', fontSize: '12px', margin: 0 }}>Advanced Bot Management</p></div>
        </div>
        <div style={{ display: 'flex', gap: '10px' }}>
          <div style={{ display: 'flex', background: '#13131f', padding: '4px', borderRadius: '10px', border: '1px solid #252535' }}>
            <button 
              onClick={() => handleModeChange("demo")}
              style={{ 
                padding: '6px 12px', fontSize: '11px', fontWeight: 700, borderRadius: '7px', 
                background: tradingMode === 'demo' ? '#3b3bff' : 'transparent', 
                border: 'none', color: tradingMode === 'demo' ? '#fff' : '#4b5563',
                cursor: 'pointer'
              }}
            >
              🎮 DEMO
            </button>
            <button 
              onClick={() => handleModeChange("live")}
              style={{ 
                padding: '6px 12px', fontSize: '11px', fontWeight: 700, borderRadius: '7px', 
                background: tradingMode === 'live' ? '#22c55e' : 'transparent', 
                border: 'none', color: tradingMode === 'live' ? '#fff' : '#4b5563',
                cursor: 'pointer'
              }}
            >
              ⚡ LIVE
            </button>
          </div>
          <button 
            onClick={() => setShowThoughts(true)}
            style={{ 
              padding: '10px 20px', background: '#6366f115', border: '1px solid #6366f130', 
              borderRadius: '10px', color: '#818cf8', fontSize: '12px', fontWeight: 600, 
              cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px' 
            }}
          >
            <Brain size={16} /> Bot Gondolatok
          </button>
          <button onClick={() => { if(confirm("PANIK: Azonnal leállítasz minden botot?")) handleBulk("stop-all") }} style={{ padding: '10px 20px', background: '#ef4444', border: 'none', borderRadius: '10px', color: '#fff', fontSize: '12px', fontWeight: 800, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px' }}><AlertCircle size={16} /> PANIK GOMB</button>
          <button style={{ padding: '10px 20px', background: '#3b3bff', border: 'none', borderRadius: '10px', color: '#fff', fontSize: '12px', fontWeight: 600, cursor: 'pointer' }}>+ Új bot</button>
        </div>
      </div>

      {/* 2. STATS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: '12px', marginBottom: '25px' }}>
        <SummaryCard label="Aktív botok" value={bots.filter(b => b.status === 'running').length} color="#a3e635" />
        <SummaryCard label="Összes PnL" value={`$${bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0).toFixed(2)}`} color={bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0) >= 0 ? "#22c55e" : "#f87171"} />
        <SummaryCard label="Összes Trade" value={bots.reduce((a, b) => a + (b.portfolio?.total_trades || 0), 0)} color="#e2e8f0" />
        <SummaryCard label="Win / Loss" value={`${bots.reduce((a, b) => a + (b.portfolio?.winning_trades || 0), 0)}W / ${bots.reduce((a, b) => a + (b.portfolio?.losing_trades || 0), 0)}L`} color="#6366f1" />
        <SummaryCard 
          label="BTC Árfolyam" 
          value={btcPrice > 0 ? `$${btcPrice.toLocaleString()}` : "Betöltés..."} 
          color="#f59e0b" 
        />
        <SummaryCard label="Egyenleg Sum" value={`$${bots.reduce((a, b) => a + (b.portfolio?.balance || 0), 0).toFixed(2)}`} color="#6366f1" />
      </div>

      {/* 2. STATS ROW 2 */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: '12px', marginBottom: '25px' }}>
        <SummaryCard 
          label="Átlag PnL / Trade" 
          value={(() => {
            const totalTrades = bots.reduce((a, b) => a + (b.portfolio?.total_trades || 0), 0);
            const totalPnl = bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0);
            return totalTrades > 0 ? `$${(totalPnl / totalTrades).toFixed(2)}` : "$0.00";
          })()} 
          color="#a5b4fc" 
        />
        <SummaryCard 
          label="Legjobb Bot" 
          value={(() => {
            const best = [...bots].sort((a, b) => (b.portfolio?.total_pnl || 0) - (a.portfolio?.total_pnl || 0))[0];
            return best && (best.portfolio?.total_pnl || 0) > 0 ? best.name : "---";
          })()} 
          color="#34d399" 
        />
        <SummaryCard 
          label="Nyitott Pozíciók" 
          value={bots.filter(checkPosition).length} 
          color="#3b82f6" 
        />
        <SummaryCard 
          label="Kitettség (Risk)" 
          value={`$${bots.filter(checkPosition).reduce((a, b) => a + b.bet_size, 0).toFixed(2)}`} 
          color="#f472b6" 
        />
        <SummaryCard 
          label="Hátralévő Idő" 
          value={timeRemaining > 0 ? `${Math.floor(timeRemaining / 60)}:${String(timeRemaining % 60).padStart(2, '0')}` : "--:--"} 
          color={timeRemaining < 60 ? "#ef4444" : "#e2e8f0"} 
        />
        <div style={{ background: '#13131f', padding: '10px 15px', borderRadius: '12px', border: '1px solid #252535' }}>
          <p style={{ fontSize: '9px', color: '#4b5563', margin: '0 0 5px', fontWeight: 700, textTransform: 'uppercase' }}>Piaci Hangulat</p>
          <div style={{ height: '6px', background: '#1a1a2e', borderRadius: '3px', overflow: 'hidden', display: 'flex', marginBottom: '6px' }}>
            <div style={{ width: `${(yesPrice || 0.5) * 100}%`, background: '#22c55e' }} />
            <div style={{ width: `${(1 - (yesPrice || 0.5)) * 100}%`, background: '#ef4444' }} />
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '9px', fontWeight: 800 }}>
            <span style={{ color: '#22c55e' }}>{((yesPrice || 0.5) * 100).toFixed(0)}% UP</span>
            <span style={{ color: '#ef4444' }}>{((1 - (yesPrice || 0.5)) * 100).toFixed(0)}% DOWN</span>
          </div>
        </div>
      </div>

      {/* 3. SZŰRŐ ÉS RENDEZŐ SÁV */}
      <div style={{ display: 'flex', gap: '12px', marginBottom: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ position: 'relative', flex: 1, minWidth: '200px' }}>
          <Search size={16} style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', color: '#4b5563' }} />
          <input type="text" placeholder="Bot keresése..." value={search} onChange={e => setSearch(e.target.value)} style={{ width: '100%', padding: '10px 10px 10px 38px', background: '#13131f', border: '1px solid #252535', borderRadius: '10px', color: '#fff', outline: 'none', fontSize: '13px' }} />
        </div>

        <div style={{ display: 'flex', gap: '5px', alignItems: 'center' }}>
          <select
            value={sortKey}
            onChange={e => setSortKey(e.target.value as SortKey)}
            style={{ background: '#13131f', color: '#fff', border: '1px solid #252535', padding: '10px 15px', borderRadius: '10px', outline: 'none', fontSize: '13px', cursor: 'pointer' }}
          >
            <option value="pnl">Rendezés: PnL</option>
            <option value="balance">Rendezés: Egyenleg</option>
            <option value="wins">Rendezés: Nyerés</option>
            <option value="losses">Rendezés: Vesztés</option>
            <option value="trades">Rendezés: Trading</option>
          </select>
          <button
            onClick={() => setSortDir(d => d === 'asc' ? 'desc' : 'asc')}
            style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '10px', padding: '10px', color: '#4b5563', cursor: 'pointer' }}
          >
            <ArrowUpDown size={16} />
          </button>
        </div>

        <button onClick={() => setQuickFilter(quickFilter === 'best3' ? 'none' : 'best3')} style={{ padding: '7px 15px', borderRadius: '8px', border: '1px solid #252535', background: quickFilter === 'best3' ? '#fbbf2415' : '#13131f', color: '#fbbf24', fontSize: '11px', fontWeight: 700 }}><Trophy size={14} style={{ marginRight: 6 }} /> Top 3 Legjobb</button>
        <button onClick={() => setQuickFilter(quickFilter === 'worst3' ? 'none' : 'worst3')} style={{ padding: '7px 15px', borderRadius: '8px', border: '1px solid #252535', background: quickFilter === 'worst3' ? '#ef444415' : '#13131f', color: '#ef4444', fontSize: '11px', fontWeight: 700 }}><AlertTriangle size={14} style={{ marginRight: 6 }} /> Top 3 Legrosszabb</button>

        <div style={{ width: '1px', height: '18px', background: '#252535', margin: '0 5px' }} />

        <div style={{ display: 'flex', alignItems: 'center', gap: '5px', background: '#13131f', padding: '5px 8px', borderRadius: '8px', border: '1px solid #252535' }}>
          <span style={{ fontSize: '10px', color: '#4b5563', fontWeight: 700, textTransform: 'uppercase' }}>Kriptó (Mind):</span>
          <select 
            onChange={async (e) => {
              const val = e.target.value;
              if (!val) return;
              if (!confirm(`Biztosan átállítod az ÖSSZES botot a(z) ${val} beállításra?`)) {
                e.target.value = "";
                return;
              }
              const loadingToast = toast.loading("Összes bot frissítése folyamatban...");
              for (const bot of bots) {
                try {
                  await apiFetch(`/bots/${bot.id}`, {
                    method: "PUT",
                    body: JSON.stringify({ market_id: val })
                  });
                } catch (err) {}
              }
              toast.dismiss(loadingToast);
              toast.success("Minden bot sikeresen átállítva!");
              loadBots();
              e.target.value = ""; 
            }}
            style={{ background: 'transparent', color: '#818cf8', border: 'none', outline: 'none', fontSize: '11px', fontWeight: 800, cursor: 'pointer' }}
          >
            <option value="">-- Válassz --</option>
            <option value="AUTO">AUTO (Körkörös)</option>
            <option value="BTC">BTC (Bitcoin)</option>
            <option value="ETH">ETH (Ethereum)</option>
            <option value="SOL">SOL (Solana)</option>
            <option value="XRP">XRP (Ripple)</option>
          </select>
        </div>

        <button onClick={() => handleBulk("start-all")} style={{ padding: '8px 15px', background: '#22c55e15', color: '#22c55e', border: '1px solid #22c55e30', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}>▶ Indít mind</button>
        <button onClick={() => handleBulk("stop-all")} style={{ padding: '8px 15px', background: '#fbbf2415', color: '#fbbf24', border: '1px solid #fbbf2430', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}>■ Megállít mind</button>
        <button onClick={() => {
          const csv = "Date,Bot,Market,Outcome,Confidence,Price,PnL\n" + bots.map(b => `${new Date().toISOString()},${b.name},${b.market_id},UP,0.85,65000,10.5`).join("\n")
          const blob = new Blob([csv], { type: 'text/csv' })
          const url = window.URL.createObjectURL(blob)
          const a = document.createElement('a')
          a.setAttribute('hidden', '')
          a.setAttribute('href', url)
          a.setAttribute('download', 'trades.csv')
          document.body.appendChild(a)
          a.click()
          document.body.removeChild(a)
        }} style={{ padding: '8px 15px', background: '#13131f', color: '#e2e8f0', border: '1px solid #252535', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}><Download size={14} style={{ marginRight: 8 }} /> Export CSV</button>
        <button onClick={() => { if (confirm("Mindent nullázol?")) handleBulk("reset-all") }} style={{ padding: '8px 15px', background: '#3b3bff15', color: '#818cf8', border: '1px solid #3b3bff30', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}><RotateCcw size={14} style={{ marginRight: 8 }} /> Statisztika nullázása</button>
      </div>

      {/* 4. AKTÍV POZÍCIÓK LISTÁJA */}
      <div style={{ marginBottom: '25px', padding: '15px', background: 'rgba(59, 130, 246, 0.03)', borderRadius: '12px', border: '1px solid rgba(59, 130, 246, 0.15)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '12px' }}><Zap size={16} color="#3b82f6" fill="#3b82f6" /><span style={{ fontSize: '12px', fontWeight: 800, color: '#3b82f6', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Aktív pozíciók a piacon</span></div>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
          {botsInPosition.length > 0 ? botsInPosition.map(bot => (
            <div key={bot.id} style={{ padding: '8px 16px', background: '#1e1e30', borderRadius: '10px', border: '1.5px solid #3b82f6', display: 'flex', alignItems: 'center', gap: '10px' }}><span style={{ fontSize: '13px', fontWeight: 700, color: '#fff' }}>{bot.name}</span><span style={{ fontSize: '9px', fontWeight: 900, color: '#3b82f6' }}>LIVE</span></div>
          )) : <span style={{ fontSize: '11px', color: '#4b5563', fontStyle: 'italic' }}>Jelenleg nincs nyitott pozíció.</span>}
        </div>
      </div>

      {/* 5. BOT RÁCS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '30px' }}>
        {filteredBots.map((bot) => (
          <BotCard key={bot.id} bot={bot} isInPos={checkPosition(bot)} isLoading={actionLoading === bot.id}
            onAction={(a: string) => handleBotAction(bot.id, a)}
            onDelete={() => { if (confirm("Törlés?")) apiFetch(`/bots/${bot.id}`, { method: "DELETE" }).then(loadBots) }}
            onEdit={() => setEditingBot(bot)}
          />
        ))}
      </div>

      <AnimatePresence>
        {editingBot && (
          <SettingsModal 
            bot={editingBot} 
            onClose={() => setEditingBot(null)} 
            onSave={(updates) => handleUpdateBot(editingBot.id, updates)}
            isUpdating={isUpdating}
          />
        )}
      </AnimatePresence>

      {/* 6. NAPLÓ */}
      <div style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '16px', padding: '18px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '15px', color: '#6366f1' }}>
          <ScrollText size={18} /><h2 style={{ fontSize: '13px', fontWeight: 700, margin: 0, textTransform: 'uppercase' }}>Eseménynapló</h2>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', maxHeight: '150px', overflowY: 'auto' }}>
          {logs.map(log => (
            <div key={log.id} style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', padding: '8px 12px', background: '#0b0b14', borderRadius: '8px', borderLeft: `3px solid ${log.type === 'success' ? '#22c55e' : '#6366f1'}` }}>
              <span>{log.msg}</span><span style={{ color: '#4b5563' }}>{log.time}</span>
            </div>
          ))}
          {logs.length === 0 && <span style={{ fontSize: '11px', color: '#333' }}>Várakozás eseményekre...</span>}
        </div>
      </div>

      <BotThoughts isOpen={showThoughts} onClose={() => setShowThoughts(false)} />
    </div>
  )
}

function BotCard({ bot, isInPos, onAction, onEdit, onDelete, isLoading }: any) {
  const pnl = bot.portfolio?.total_pnl || 0; const balance = bot.portfolio?.balance || 0;
  const strategyColor = STRATEGY_COLORS[bot.strategy_type] || '#818cf8'

  // ---- ÉLŐ SZÁMLÁLÓK ----
  const runElapsed = useElapsedTimer(bot.runSince)
  const posElapsed = useElapsedTimer(bot.posSince)

  return (
    <motion.div layout style={{
      background: '#13131f', border: isInPos ? '1.5px solid #22c55e' : '1px solid #252535',
      borderRadius: '16px', padding: '15px', display: 'flex', flexDirection: 'column', gap: '10px',
      backgroundColor: isInPos ? 'rgba(34, 197, 94, 0.05)' : '#13131f', position: 'relative'
    }}>
      {/* Trading Mode Badge */}
      <div style={{ 
        position: 'absolute', 
        top: '10px', 
        left: '10px', 
        background: bot.trading_mode === 'live' ? '#ef4444' : '#3b82f6', 
        color: '#fff', 
        fontSize: '7px', 
        fontWeight: 900, 
        padding: '1px 5px', 
        borderRadius: '4px',
        zIndex: 10,
        boxShadow: bot.trading_mode === 'live' ? '0 0 10px rgba(239, 68, 68, 0.3)' : 'none'
      }}>
        {bot.trading_mode === 'live' ? 'ÉLES (LIVE)' : 'DEMO'}
      </div>

      {isInPos && (
        <div style={{ 
          position: 'absolute', 
          top: '10px', 
          right: '10px', 
          background: '#22c55e', 
          color: '#000', 
          fontSize: '8px', 
          fontWeight: 900, 
          padding: '2px 6px', 
          borderRadius: '4px',
          animation: 'pulse 2s infinite'
        }}>
          POZÍCIÓBAN
        </div>
      )}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h3 style={{ fontSize: '13px', fontWeight: 700, margin: '15px 0 0' }}>{bot.name}</h3>
        <div style={{ width: '7px', height: '7px', borderRadius: '50%', background: bot.status === 'running' ? '#22c55e' : '#4b5563' }} />
      </div>
      <span style={{ fontSize: '8px', fontWeight: 800, color: strategyColor, background: `${strategyColor}15`, padding: '2px 6px', borderRadius: '4px', alignSelf: 'flex-start' }}>{bot.strategy_type.toUpperCase()}</span>

      {/* ---- SZÁMLÁLÓK BLOKK ---- */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '5px' }}>
        {/* Futási idő számláló */}
        <div style={{
          background: bot.status === 'running' ? 'rgba(99,102,241,0.08)' : '#0d0d1a',
          border: `1px solid ${bot.status === 'running' ? '#3b3bff40' : '#1e1e30'}`,
          borderRadius: '8px', padding: '6px 8px'
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px', marginBottom: '2px' }}>
            <Clock size={9} color={bot.status === 'running' ? '#818cf8' : '#4b5563'} />
            <span style={{ fontSize: '7px', fontWeight: 800, color: bot.status === 'running' ? '#818cf8' : '#4b5563', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Fut</span>
          </div>
          <span style={{ fontSize: '10px', fontWeight: 700, color: bot.status === 'running' ? '#a5b4fc' : '#4b5563', fontVariantNumeric: 'tabular-nums' }}>
            {bot.status === 'running' ? runElapsed : '—'}
          </span>
        </div>

        {/* Pozíció idő számláló */}
        <div style={{
          background: isInPos ? 'rgba(34,197,94,0.08)' : '#0d0d1a',
          border: `1px solid ${isInPos ? '#22c55e40' : '#1e1e30'}`,
          borderRadius: '8px', padding: '6px 8px'
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px', marginBottom: '2px' }}>
            <Zap size={9} color={isInPos ? '#22c55e' : '#4b5563'} />
            <span style={{ fontSize: '7px', fontWeight: 800, color: isInPos ? '#22c55e' : '#4b5563', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Pozíció</span>
          </div>
          <span style={{ fontSize: '10px', fontWeight: 700, color: isInPos ? '#4ade80' : '#4b5563', fontVariantNumeric: 'tabular-nums' }}>
            {isInPos ? posElapsed : '—'}
          </span>
        </div>
      </div>

      <div style={{ background: '#0d0d1a', padding: '10px', borderRadius: '12px', border: '1px solid #1e1e30' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '5px' }}><span style={{ fontSize: '8px', color: '#4b5563', fontWeight: 700 }}>EGYENLEG</span><span style={{ fontSize: '12px', fontWeight: 700 }}>${balance.toFixed(2)}</span></div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}><span style={{ fontSize: '8px', color: '#4b5563', fontWeight: 700 }}>PnL</span><span style={{ fontSize: '14px', fontWeight: 800, color: pnl >= 0 ? '#4ade80' : '#f87171' }}>{pnl >= 0 ? '+' : ''}${pnl.toFixed(2)}</span></div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '5px' }}>
        <div style={{ background: '#1a1a2e', padding: '4px', borderRadius: '6px', textAlign: 'center' }}><p style={{ fontSize: '6px', color: '#4b5563', margin: 0 }}>TÉT</p><p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>${bot.bet_size}</p></div>
        <div style={{ background: '#1a1a2e', padding: '4px', borderRadius: '6px', textAlign: 'center' }}><p style={{ fontSize: '6px', color: '#4b5563', margin: 0 }}>SL</p><p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>-10%</p></div>
        <div style={{ background: '#1a1a2e', padding: '4px', borderRadius: '6px', textAlign: 'center' }}><p style={{ fontSize: '6px', color: '#4b5563', margin: 0 }}>TP</p><p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>+20%</p></div>
      </div>

      <div style={{ marginTop: '8px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '8px', color: '#4b5563', marginBottom: '3px', fontWeight: 800, textTransform: 'uppercase' }}><span>Teljesítmény</span></div>
        <div style={{ height: '45px', background: '#080812', borderRadius: '10px', border: '1px solid #1e1e30', overflow: 'hidden', position: 'relative' }}>
          <Sparkline data={bot.pnl_history || []} />
        </div>
      </div>

      <div style={{ marginTop: '8px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '8px', color: '#4b5563', marginBottom: '4px', fontWeight: 800, textTransform: 'uppercase' }}>
          <span>UTÓBBI KÖTÉSEK</span>
          <span style={{ color: '#6366f1' }}>{bot.portfolio?.winning_trades || 0}W / {bot.portfolio?.losing_trades || 0}L</span>
        </div>
        <div style={{ height: '50px', background: '#080812', borderRadius: '10px', border: '1px solid #1e1e30', padding: '6px', overflowY: 'auto' }}>
          {(bot.pnl_history || []).slice().reverse().map((pnl: number, idx: number) => (
            <div key={idx} style={{ display: 'flex', justifyContent: 'space-between', fontSize: '9px', marginBottom: '3px', padding: '2px 5px', borderRadius: '4px', background: pnl > 0 ? '#22c55e08' : (pnl < 0 ? '#ef444408' : 'transparent') }}>
              <span style={{ color: pnl > 0 ? '#22c55e' : (pnl < 0 ? '#ef4444' : '#4b5563'), fontWeight: 700 }}>
                {pnl > 0 ? '✅ NYERESÉG' : (pnl < 0 ? '❌ VESZTESÉG' : '⏳ TARTÁS')}
              </span>
              <span style={{ color: '#fafafa', opacity: 0.8 }}>${Math.abs(pnl).toFixed(2)}</span>
            </div>
          ))}
          {(!bot.pnl_history || bot.pnl_history.length === 0) && <p style={{ fontSize: '8px', color: '#333', textAlign: 'center', marginTop: '12px' }}>Még nincs kötés...</p>}
        </div>
      </div>

      <div style={{ height: '3px', background: '#1e1e30', borderRadius: '2px', overflow: 'hidden' }}><div style={{ height: '100%', width: `${bot.portfolio?.win_rate || 0}%`, background: '#22c55e' }} /></div>

      <div style={{ display: 'flex', gap: '5px' }}>
        <button onClick={() => onAction(bot.status === 'running' ? 'stop' : 'start')} disabled={isLoading} style={{ flex: 3, padding: '10px', background: bot.status === 'running' ? '#fbbf2415' : '#22c55e15', color: bot.status === 'running' ? '#fbbf24' : '#22c55e', border: 'none', borderRadius: '10px', cursor: 'pointer', fontSize: '10px', fontWeight: 800 }}>{isLoading ? '...' : (bot.status === 'running' ? 'LEÁLLÍT' : 'INDÍTÁS')}</button>
        <button onClick={() => onAction('reset')} style={{ flex: 1, padding: '10px', background: '#3b3bff15', color: '#818cf8', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><RotateCcw size={14} /></button>
        <button onClick={onEdit} style={{ flex: 1, padding: '10px', background: '#6366f115', color: '#818cf8', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><Settings size={14} /></button>
        <button onClick={onDelete} style={{ flex: 1, padding: '10px', background: '#ef444415', color: '#ef4444', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><Trash2 size={14} /></button>
      </div>
    </motion.div>
  )
}

function SummaryCard({ label, value, color }: { label: string, value: string | number, color: string }) {
  return (
    <div style={{ background: '#13131f', padding: '10px 15px', borderRadius: '12px', border: '1px solid #252535' }}>
      <p style={{ fontSize: '9px', color: '#4b5563', margin: '0 0 5px', fontWeight: 700, textTransform: 'uppercase' }}>{label}</p>
      <p style={{ fontSize: '16px', fontWeight: 700, color: color, margin: 0 }}>{value}</p>
    </div>
  )
}

function Sparkline({ data }: { data: number[] }) {
  if (!data || data.length < 2) return <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '8px', color: '#333' }}>Várakozás adatokra...</div>
  
  // Számoljuk ki a kumulált profitot a vizualizációhoz
  let cumulative = 0;
  const points_data = data.map(v => {
    cumulative += v;
    return cumulative;
  });

  const min = Math.min(...points_data); const max = Math.max(...points_data);
  const range = max - min || 1; const width = 200; const height = 45;
  
  const points = points_data.map((v, i) => {
    const x = (i / (points_data.length - 1)) * width
    const y = height - ((v - min) / range) * height
    return `${x},${y}`
  }).join(' ')

  const isUp = points_data[points_data.length - 1] >= points_data[0]

  return (
    <svg width="100%" height="100%" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" style={{ filter: 'drop-shadow(0 0 2px rgba(99, 102, 241, 0.2))' }}>
      <polyline fill="none" stroke={isUp ? "#22c55e" : "#ef4444"} strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" points={points} style={{ transition: 'all 0.5s ease' }} />
      <path d={`M ${points} L ${width},${height} L 0,${height} Z`} fill={isUp ? "url(#gradUp)" : "url(#gradDown)"} opacity="0.15" />
      <defs>
        <linearGradient id="gradUp" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" style={{ stopColor: '#22c55e' }} /><stop offset="100%" style={{ stopColor: 'transparent' }} /></linearGradient>
        <linearGradient id="gradDown" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" style={{ stopColor: '#ef4444' }} /><stop offset="100%" style={{ stopColor: 'transparent' }} /></linearGradient>
      </defs>
    </svg>
  )
}

function SettingsModal({ bot, onClose, onSave, isUpdating }: { bot: Bot, onClose: () => void, onSave: (u: any) => void, isUpdating: boolean }) {
  const [betSize, setBetSize] = useState(bot.bet_size)
  const [stopLoss, setStopLoss] = useState(bot.stop_loss)
  const [takeProfit, setTakeProfit] = useState(bot.take_profit)
  const [name, setName] = useState(bot.name)
  const [marketId, setMarketId] = useState(bot.market_id || 'AUTO')

  return (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.8)', backdropFilter: 'blur(4px)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100, padding: '20px' }}>
      <motion.div initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '24px', width: '100%', maxWidth: '400px', overflow: 'hidden' }}>
        <div style={{ padding: '20px', borderBottom: '1px solid #252535', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}><Settings size={18} color="#6366f1" /><h2 style={{ fontSize: '16px', fontWeight: 700, margin: 0 }}>Bot Beállítások</h2></div>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#4b5563', cursor: 'pointer' }}><X size={20} /></button>
        </div>
        
        <div style={{ padding: '25px', display: 'flex', flexDirection: 'column', gap: '20px' }}>
          <div>
            <label style={{ display: 'block', fontSize: '11px', fontWeight: 700, color: '#4b5563', marginBottom: '8px', textTransform: 'uppercase' }}>Bot Neve</label>
            <input type="text" value={name} onChange={e => setName(e.target.value)} style={{ width: '100%', background: '#0b0b14', border: '1px solid #252535', borderRadius: '12px', padding: '12px', color: '#fff', fontSize: '14px', outline: 'none' }} />
          </div>

          <div>
            <label style={{ display: 'block', fontSize: '11px', fontWeight: 700, color: '#4b5563', marginBottom: '8px', textTransform: 'uppercase' }}>Kriptovaluta</label>
            <select value={marketId} onChange={e => setMarketId(e.target.value)} style={{ width: '100%', background: '#0b0b14', border: '1px solid #252535', borderRadius: '12px', padding: '12px', color: '#fff', fontSize: '14px', outline: 'none', cursor: 'pointer' }}>
              <option value="AUTO">Automatikus (Round-Robin)</option>
              <option value="BTC">Bitcoin (BTC)</option>
              <option value="ETH">Ethereum (ETH)</option>
              <option value="SOL">Solana (SOL)</option>
              <option value="XRP">Ripple (XRP)</option>
            </select>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
            <div>
              <label style={{ display: 'block', fontSize: '11px', fontWeight: 700, color: '#4b5563', marginBottom: '8px', textTransform: 'uppercase' }}>Alap Tét ($)</label>
              <input type="number" step="0.1" value={betSize} onChange={e => setBetSize(Number(e.target.value))} style={{ width: '100%', background: '#0b0b14', border: '1px solid #252535', borderRadius: '12px', padding: '12px', color: '#fff', fontSize: '14px', outline: 'none' }} />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '11px', fontWeight: 700, color: '#4b5563', marginBottom: '8px', textTransform: 'uppercase' }}>Stop Loss (%)</label>
              <input type="number" step="0.01" value={stopLoss} onChange={e => setStopLoss(Number(e.target.value))} style={{ width: '100%', background: '#0b0b14', border: '1px solid #252535', borderRadius: '12px', padding: '12px', color: '#fff', fontSize: '14px', outline: 'none' }} />
            </div>
          </div>

          <div>
            <label style={{ display: 'block', fontSize: '11px', fontWeight: 700, color: '#4b5563', marginBottom: '8px', textTransform: 'uppercase' }}>Take Profit (%)</label>
            <input type="number" step="0.01" value={takeProfit} onChange={e => setTakeProfit(Number(e.target.value))} style={{ width: '100%', background: '#0b0b14', border: '1px solid #252535', borderRadius: '12px', padding: '12px', color: '#fff', fontSize: '14px', outline: 'none' }} />
          </div>

          <div style={{ marginTop: '10px', display: 'flex', gap: '10px' }}>
            <button onClick={onClose} style={{ flex: 1, padding: '14px', background: 'transparent', border: '1px solid #252535', color: '#fff', borderRadius: '12px', cursor: 'pointer', fontSize: '13px', fontWeight: 600 }}>Mégse</button>
            <button onClick={() => onSave({ name, bet_size: betSize, stop_loss: stopLoss, take_profit: takeProfit, market_id: marketId })} disabled={isUpdating} style={{ flex: 2, padding: '14px', background: '#3b3bff', border: 'none', color: '#fff', borderRadius: '12px', cursor: 'pointer', fontSize: '13px', fontWeight: 700, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
              {isUpdating ? <Loader2 size={18} className="animate-spin" /> : 'Beállítások mentése'}
            </button>
          </div>
        </div>
      </motion.div>
    </motion.div>
  )
}
