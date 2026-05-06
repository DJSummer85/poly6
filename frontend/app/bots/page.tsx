'use client'

import { useState, useMemo, useEffect, useCallback } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Activity, Bot as BotIcon, Loader2, Play, Plus, Square, Trash2, RotateCcw,
  Shield, Target, Wallet, Search, ArrowUpDown, Wifi, WifiOff, Trophy, AlertTriangle, X
} from "lucide-react"
import { toast } from "sonner"
import { apiFetch } from "@/lib/utils"
import { useAppStore } from "@/store"

// ---- Típusok ----
type BotStatus = 'running' | 'paused' | 'error' | 'stopped'
type SortKey = 'pnl' | 'winRate' | 'trades' | 'balance' | 'name'

interface Bot {
  id: string
  name: string
  strategy_type: string
  status: BotStatus
  trading_mode: 'paper' | 'live'
  bet_size: number
  stop_loss: number
  take_profit: number
  market_id: string
  portfolio?: {
    balance: number
    total_pnl: number
    total_trades: number
    winning_trades: number
    losing_trades: number
    win_rate: number
  }
}

const STRATEGY_COLORS: Record<string, string> = {
  momentum: '#818cf8',
  mean_reversion: '#34d399',
  last_seconds_scalp: '#f472b6',
  binance_signal: '#38bdf8',
  scalping: '#fb923c'
}

const SORT_OPTIONS: { key: SortKey; label: string }[] = [
  { key: 'pnl', label: 'Legtöbb nyereség (PnL)' },
  { key: 'balance', label: 'Legmagasabb egyenleg' },
  { key: 'winRate', label: 'Win rate' },
  { key: 'trades', label: 'Legtöbb trade' },
  { key: 'name', label: 'Név (A-Z)' },
]

export default function BotsPage() {
  const [bots, setBots] = useState<Bot[]>([])
  const [actionLoading, setActionLoading] = useState<string | null>(null)
  const [lastSync, setLastSync] = useState<Date>(new Date())
  const [isSyncing, setIsSyncing] = useState(false)
  const [serverOnline, setServerOnline] = useState(true)

  // UI Állapotok
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | BotStatus>('all')
  const [sortKey, setSortKey] = useState<SortKey>('pnl')
  const [sortDir, setSortDir] = useState<'desc' | 'asc'>('desc')
  const [quickFilter, setQuickFilter] = useState<'none' | 'best3' | 'worst3'>('none')

  // ---- Adatbetöltés ----
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
      setBots(withPortfolio)
      setLastSync(new Date())
      setServerOnline(true)
    } catch (err) {
      setServerOnline(false)
    } finally {
      setIsSyncing(false)
    }
  }, [])

  useEffect(() => {
    loadBots()
    const interval = setInterval(loadBots, 20000)
    return () => clearInterval(interval)
  }, [loadBots])

  // ---- Műveletek ----
  const handleStart = async (id: string) => {
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/start`, { method: "POST", body: JSON.stringify({ initial_balance: 100 }) })
      toast.success("Bot elindítva")
      await loadBots()
    } catch (err: any) { toast.error(err.message) }
    finally { setActionLoading(null) }
  }

  const handleStop = async (id: string) => {
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/stop`, { method: "POST" })
      toast.success("Bot leállítva")
      await loadBots()
    } catch (err: any) { toast.error(err.message) }
    finally { setActionLoading(null) }
  }

  const handleReset = async (id: string) => {
    if (!confirm("Biztosan nullázod ezt a botot?")) return
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/reset`, { method: "POST" })
      toast.success("Bot statisztikák nullázva")
      await loadBots()
    } catch (err) { toast.error("Reset nem elérhető a backendben") }
    finally { setActionLoading(null) }
  }

  const handleBulkAction = async (action: 'start' | 'stop') => {
    const targets = bots.filter(b => action === 'start' ? b.status !== 'running' : b.status === 'running')
    if (targets.length === 0) return toast.info("Nincs módosítható bot")
    toast.promise(Promise.all(targets.map(b => apiFetch(`/bots/${b.id}/${action}`, { method: "POST" }))), {
      loading: "Művelet folyamatban...",
      success: "Összes bot frissítve!",
      error: "Hiba történt"
    })
    setTimeout(loadBots, 2000)
  }

  // ---- Szűrés és Rendezés ----
  const filteredBots = useMemo(() => {
    let list = [...bots]
    if (search) list = list.filter(b => b.name.toLowerCase().includes(search.toLowerCase()))
    if (statusFilter !== 'all') list = list.filter(b => b.status === statusFilter)

    list.sort((a, b) => {
      let valA: any = 0; let valB: any = 0
      if (sortKey === 'pnl') { valA = a.portfolio?.total_pnl || 0; valB = b.portfolio?.total_pnl || 0 }
      else if (sortKey === 'balance') { valA = a.portfolio?.balance || 0; valB = b.portfolio?.balance || 0 }
      else if (sortKey === 'winRate') { valA = a.portfolio?.win_rate || 0; valB = b.portfolio?.win_rate || 0 }
      else if (sortKey === 'trades') { valA = a.portfolio?.total_trades || 0; valB = b.portfolio?.total_trades || 0 }
      else if (sortKey === 'name') return sortDir === 'desc' ? b.name.localeCompare(a.name) : a.name.localeCompare(b.name)
      return sortDir === 'desc' ? valB - valA : valA - valB
    })

    if (quickFilter === 'best3') return [...list].sort((a, b) => (b.portfolio?.total_pnl || 0) - (a.portfolio?.total_pnl || 0)).slice(0, 3)
    if (quickFilter === 'worst3') return [...list].sort((a, b) => (a.portfolio?.total_pnl || 0) - (b.portfolio?.total_pnl || 0)).slice(0, 3)
    return list
  }, [bots, search, statusFilter, sortKey, sortDir, quickFilter])

  // Összesített statisztikák a felső kártyákhoz
  const totalStats = {
    active: bots.filter(b => b.status === 'running').length,
    pnl: bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0),
    balance: bots.reduce((a, b) => a + (b.portfolio?.balance || 0), 0),
    trades: bots.reduce((a, b) => a + (b.portfolio?.total_trades || 0), 0),
    wins: bots.reduce((a, b) => a + (b.portfolio?.winning_trades || 0), 0),
    losses: bots.reduce((a, b) => a + (b.portfolio?.losing_trades || 0), 0),
  }

  return (
    <div style={{ padding: '24px', background: '#0b0b14', minHeight: '100vh', color: '#e2e8f0', fontFamily: 'system-ui, sans-serif' }}>

      {/* 1. FEJLÉC */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '25px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '15px' }}>
          <div style={{ background: '#1a1a2e', padding: '10px', borderRadius: '12px', border: '1px solid #252535' }}><BotIcon size={24} color="#6366f1" /></div>
          <div>
            <h1 style={{ fontSize: '22px', fontWeight: 700, margin: 0 }}>Botok</h1>
            <p style={{ color: '#4b5563', fontSize: '13px', margin: 0 }}>Trading botok kezelése</p>
          </div>
        </div>
        <div style={{ display: 'flex', gap: '10px' }}>
          <button onClick={() => toast.info("Összes nullázása...")} style={{ padding: '10px 15px', background: '#1a1a2e', border: '1px solid #252535', borderRadius: '10px', color: '#e2e8f0', fontSize: '13px', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <RotateCcw size={14} /> Összes nullázása
          </button>
          <button style={{ padding: '10px 20px', background: '#3b3bff', border: 'none', borderRadius: '10px', color: '#fff', fontSize: '13px', fontWeight: 600, cursor: 'pointer' }}>+ Új bot</button>
        </div>
      </div>

      {/* 2. STATISZTIKAI KÁRTYÁK */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '25px' }}>
        <SummaryCard label="Aktív botok" value={totalStats.active} color="#a3e635" />
        <SummaryCard label="Összes PnL" value={`$${totalStats.pnl.toFixed(2)}`} color={totalStats.pnl >= 0 ? "#4ade80" : "#f87171"} />
        <SummaryCard label="Trades" value={totalStats.trades} color="#e2e8f0" />
        <div style={{ background: '#13131f', padding: '15px', borderRadius: '12px', border: '1px solid #252535' }}>
          <p style={{ fontSize: '11px', color: '#4b5563', margin: '0 0 8px' }}>Nyerés / Vesztés</p>
          <p style={{ fontSize: '20px', fontWeight: 700, margin: 0 }}>
            <span style={{ color: '#4ade80' }}>{totalStats.wins}W</span>
            <span style={{ color: '#4b5563', margin: '0 5px' }}>/</span>
            <span style={{ color: '#f87171' }}>{totalStats.losses}L</span>
          </p>
        </div>
        <SummaryCard label="Avg Win Rate" value={`${(totalStats.trades > 0 ? (totalStats.wins / totalStats.trades) * 100 : 0).toFixed(1)}%`} color="#6366f1" />
      </div>

      {/* 3. SZŰRŐ ÉS RENDEZŐ SÁV */}
      <div style={{ display: 'flex', gap: '12px', marginBottom: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ position: 'relative', flex: 1, minWidth: '200px' }}>
          <Search size={16} style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', color: '#4b5563' }} />
          <input type="text" placeholder="Bot keresése..." value={search} onChange={e => setSearch(e.target.value)} style={{ width: '100%', padding: '10px 10px 10px 38px', background: '#13131f', border: '1px solid #252535', borderRadius: '10px', color: '#fff', outline: 'none', fontSize: '13px' }} />
        </div>

        {/* Státusz szűrő */}
        <div style={{ display: 'flex', background: '#13131f', padding: '3px', borderRadius: '10px', border: '1px solid #252535' }}>
          {(['all', 'running', 'stopped', 'error'] as const).map(f => (
            <button key={f} onClick={() => setStatusFilter(f)} style={{ padding: '7px 14px', fontSize: '12px', borderRadius: '8px', border: 'none', background: statusFilter === f ? '#3b3bff20' : 'transparent', color: statusFilter === f ? '#818cf8' : '#4b5563', cursor: 'pointer' }}>
              {f === 'all' ? 'Összes' : f === 'running' ? '● Aktív' : f === 'stopped' ? '■ Leállítva' : '✕ Hiba'}
            </button>
          ))}
        </div>

        {/* RENDEZŐ DROPDOWN */}
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <select value={sortKey} onChange={e => setSortKey(e.target.value as SortKey)} style={{ background: '#13131f', color: '#fff', border: '1px solid #252535', padding: '10px 15px', borderRadius: '10px', outline: 'none', fontSize: '13px', cursor: 'pointer' }}>
            {SORT_OPTIONS.map(opt => <option key={opt.key} value={opt.key}>{opt.label}</option>)}
          </select>
          <button onClick={() => setSortDir(d => d === 'asc' ? 'desc' : 'asc')} style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '10px', padding: '10px', color: '#4b5563', cursor: 'pointer' }}>
            <ArrowUpDown size={16} />
          </button>
        </div>
      </div>

      {/* 4. GYORS MŰVELETEK ÉS TOP SZŰRŐK */}
      <div style={{ display: 'flex', gap: '10px', marginBottom: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
        <span style={{ fontSize: '11px', fontWeight: 700, color: '#4b5563', textTransform: 'uppercase' }}>Gyors elérés:</span>
        <button onClick={() => handleBulkAction('start')} style={{ padding: '7px 15px', background: '#22c55e15', color: '#22c55e', border: '1px solid #22c55e30', borderRadius: '8px', fontSize: '12px', fontWeight: 600, cursor: 'pointer' }}>▶ Összes indítása</button>
        <button onClick={() => handleBulkAction('stop')} style={{ padding: '7px 15px', background: '#fbbf2415', color: '#fbbf24', border: '1px solid #fbbf2430', borderRadius: '8px', fontSize: '12px', fontWeight: 600, cursor: 'pointer' }}>■ Összes leállítása</button>

        <div style={{ width: '1px', height: '18px', background: '#252535', margin: '0 5px' }} />

        <button onClick={() => setQuickFilter(quickFilter === 'best3' ? 'none' : 'best3')} style={{ display: 'flex', alignItems: 'center', gap: '6px', padding: '7px 15px', borderRadius: '8px', border: '1px solid #252535', background: quickFilter === 'best3' ? '#fbbf2415' : '#13131f', color: quickFilter === 'best3' ? '#fbbf24' : '#6b7280', fontSize: '12px', cursor: 'pointer' }}>
          <Trophy size={14} /> Top 3 Legjobb
        </button>
        <button onClick={() => setQuickFilter(quickFilter === 'worst3' ? 'none' : 'worst3')} style={{ display: 'flex', alignItems: 'center', gap: '6px', padding: '7px 15px', borderRadius: '8px', border: '1px solid #252535', background: quickFilter === 'worst3' ? '#ef444415' : '#13131f', color: quickFilter === 'worst3' ? '#ef4444' : '#6b7280', fontSize: '12px', cursor: 'pointer' }}>
          <AlertTriangle size={14} /> Top 3 Legrosszabb
        </button>
      </div>

      {/* 5. BOT RÁCS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px' }}>
        {filteredBots.map((bot) => (
          <BotCard
            key={bot.id}
            bot={bot}
            isLoading={actionLoading === bot.id}
            onStart={() => handleStart(bot.id)}
            onStop={() => handleStop(bot.id)}
            onReset={() => handleReset(bot.id)}
            onDelete={() => { if (confirm("Törlöd a botot?")) apiFetch(`/bots/${bot.id}`, { method: "DELETE" }).then(loadBots) }}
          />
        ))}
      </div>

      {/* Állapotjelző */}
      <div style={{ position: 'fixed', bottom: '20px', left: '20px', display: 'flex', alignItems: 'center', gap: '10px', background: '#13131f', padding: '8px 15px', borderRadius: '20px', border: '1px solid #252535', fontSize: '11px', color: '#4b5563' }}>
        {serverOnline ? <Wifi size={14} color="#22c55e" /> : <WifiOff size={14} color="#ef4444" />}
        <span>Utolsó frissítés: {lastSync.toLocaleTimeString()}</span>
        {isSyncing && <Loader2 size={12} className="animate-spin" />}
      </div>
    </div>
  )
}

function SummaryCard({ label, value, color }: { label: string, value: string | number, color: string }) {
  return (
    <div style={{ background: '#13131f', padding: '15px', borderRadius: '12px', border: '1px solid #252535' }}>
      <p style={{ fontSize: '11px', color: '#4b5563', margin: '0 0 8px' }}>{label}</p>
      <p style={{ fontSize: '20px', fontWeight: 700, color: color, margin: 0 }}>{value}</p>
    </div>
  )
}

function BotCard({ bot, onStart, onStop, onReset, onDelete, isLoading }: { bot: Bot, onStart: any, onStop: any, onReset: any, onDelete: any, isLoading: boolean }) {
  const pnl = bot.portfolio?.total_pnl || 0
  const balance = bot.portfolio?.balance || 0
  const strategyColor = STRATEGY_COLORS[bot.strategy_type] || '#818cf8'

  return (
    <motion.div layout initial={{ opacity: 0 }} animate={{ opacity: 1 }} style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '16px', padding: '15px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ overflow: 'hidden' }}>
          <h3 style={{ fontSize: '13px', fontWeight: 700, margin: 0, whiteSpace: 'nowrap', textOverflow: 'ellipsis' }}>{bot.name}</h3>
          <span style={{ fontSize: '9px', fontWeight: 800, color: strategyColor, background: `${strategyColor}15`, padding: '2px 6px', borderRadius: '4px', marginTop: '4px', display: 'inline-block' }}>{bot.strategy_type.toUpperCase()}</span>
        </div>
        <div style={{ width: '8px', height: '8px', borderRadius: '50%', background: bot.status === 'running' ? '#22c55e' : '#4b5563', boxShadow: bot.status === 'running' ? '0 0 10px #22c55e' : 'none' }} />
      </div>

      <div style={{ background: '#0d0d1a', padding: '10px', borderRadius: '12px', border: '1px solid #1e1e30' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '5px' }}>
          <span style={{ fontSize: '9px', color: '#4b5563', fontWeight: 600 }}>EGYENLEG</span>
          <span style={{ fontSize: '13px', fontWeight: 700 }}>${balance.toFixed(2)}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <span style={{ fontSize: '9px', color: '#4b5563', fontWeight: 600 }}>PnL</span>
          <span style={{ fontSize: '15px', fontWeight: 800, color: pnl >= 0 ? '#4ade80' : '#f87171' }}>{pnl >= 0 ? '+' : ''}${pnl.toFixed(2)}</span>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '5px' }}>
        <div style={{ background: '#1a1a2e', padding: '6px', borderRadius: '8px', textAlign: 'center', border: '1px solid #252535' }}>
          <p style={{ fontSize: '8px', color: '#4b5563', margin: '0 0 2px' }}>TÉT</p>
          <p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>${bot.bet_size}</p>
        </div>
        <div style={{ background: '#ef444408', padding: '6px', borderRadius: '8px', textAlign: 'center', border: '1px solid #ef444420' }}>
          <p style={{ fontSize: '8px', color: '#ef4444', margin: '0 0 2px' }}>SL</p>
          <p style={{ fontSize: '10px', fontWeight: 700, margin: 0, color: '#ef4444' }}>-{(bot.stop_loss * 100).toFixed(0)}%</p>
        </div>
        <div style={{ background: '#22c55e08', padding: '6px', borderRadius: '8px', textAlign: 'center', border: '1px solid #22c55e20' }}>
          <p style={{ fontSize: '8px', color: '#22c55e', margin: '0 0 2px' }}>TP</p>
          <p style={{ fontSize: '10px', fontWeight: 700, margin: 0, color: '#22c55e' }}>+{(bot.take_profit * 100).toFixed(0)}%</p>
        </div>
      </div>

      <div style={{ display: 'flex', gap: '5px', marginTop: '8px' }}>
        {bot.status === 'running' ? (
          <button onClick={onStop} disabled={isLoading} style={{ flex: 3, padding: '10px', background: '#fbbf2415', color: '#fbbf24', border: '1px solid #fbbf2430', borderRadius: '10px', cursor: 'pointer', fontSize: '11px', fontWeight: 800 }}>
            {isLoading ? <Loader2 size={14} className="animate-spin" /> : 'LEÁLLÍT'}
          </button>
        ) : (
          <button onClick={onStart} disabled={isLoading} style={{ flex: 3, padding: '10px', background: '#22c55e15', color: '#22c55e', border: '1px solid #22c55e30', borderRadius: '10px', cursor: 'pointer', fontSize: '11px', fontWeight: 800 }}>
            {isLoading ? <Loader2 size={14} className="animate-spin" /> : 'INDÍTÁS'}
          </button>
        )}
        <button onClick={onReset} title="Nullázás" disabled={isLoading} style={{ flex: 1, padding: '10px', background: '#3b3bff15', color: '#818cf8', border: '1px solid #3b3bff30', borderRadius: '10px', cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <RotateCcw size={16} />
        </button>
        <button onClick={onDelete} title="Törlés" style={{ flex: 1, padding: '10px', background: '#ef444415', color: '#ef4444', border: '1px solid #ef444430', borderRadius: '10px', cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <Trash2 size={16} />
        </button>
      </div>
    </motion.div>
  )
}