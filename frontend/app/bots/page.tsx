'use client'

import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Activity, Bot as BotIcon, Loader2, Play, Plus, Square, Trash2, RotateCcw,
  Shield, Target, Wallet, Search, ArrowUpDown, Wifi, WifiOff, Trophy, AlertTriangle,
  X, TrendingUp, ScrollText, Clock, History, Zap, ChevronDown
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
  history?: any[]
  portfolio?: {
    balance: number
    initial_balance: number
    total_pnl: number
    total_trades: number
    winning_trades: number
    losing_trades: number
    win_rate: number
    open_positions: number
  }
}

const STRATEGY_COLORS: Record<string, string> = {
  momentum: '#818cf8',
  mean_reversion: '#34d399',
  last_seconds_scalp: '#f472b6',
  binance_signal: '#38bdf8',
  scalping: '#fb923c'
}

export default function BotsPage() {
  const [bots, setBots] = useState<Bot[]>([])
  const [logs, setLogs] = useState<any[]>([])
  const [actionLoading, setActionLoading] = useState<string | null>(null)
  const [lastSync, setLastSync] = useState<Date>(new Date())
  const [isSyncing, setIsSyncing] = useState(false)
  const [serverOnline, setServerOnline] = useState(true)
  const [mounted, setMounted] = useState(false)
  const prevBotsRef = useRef<Bot[]>([])

  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | BotStatus>('all')
  const [sortKey, setSortKey] = useState<SortKey>('pnl')
  const [sortDir, setSortDir] = useState<'desc' | 'asc'>('desc')
  const [quickFilter, setQuickFilter] = useState<'none' | 'best3' | 'worst3'>('none')

  // POZÍCIÓ ELLENŐRZÉSE (Demo barát verzió)
  const checkPosition = (bot: Bot) => {
    if (!bot.portfolio) return false;
    // 1. Ha a backend azt mondja, van nyitott pozíció (Live)
    if (bot.portfolio.open_positions > 0) return true;
    // 2. Ha az aktuális egyenleg nem egyezik a kezdővel, és a bot fut (Demo/Paper trade indikátor)
    if (bot.status === 'running' && Math.abs(bot.portfolio.balance - bot.portfolio.initial_balance) > 0.01) return true;
    return false;
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
    setMounted(true)
    loadBots()
    const interval = setInterval(loadBots, 15000)
    return () => clearInterval(interval)
  }, [loadBots])

  const botsInPosition = useMemo(() => {
    return bots.filter(bot => checkPosition(bot));
  }, [bots]);

  const filteredBots = useMemo(() => {
    let list = [...bots]
    if (search) list = list.filter(b => b.name.toLowerCase().includes(search.toLowerCase()))
    if (statusFilter !== 'all') list = list.filter(b => b.status === statusFilter)

    list.sort((a, b) => {
      let valA = a.portfolio?.total_pnl || 0;
      let valB = b.portfolio?.total_pnl || 0;
      return sortDir === 'desc' ? valB - valA : valA - valB
    })
    return list
  }, [bots, search, statusFilter, sortDir])

  const totalStats = {
    active: bots.filter(b => b.status === 'running').length,
    pnl: bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0),
    balance: bots.reduce((a, b) => a + (b.portfolio?.balance || 0), 0),
    trades: bots.reduce((a, b) => a + (b.portfolio?.total_trades || 0), 0),
    wins: bots.reduce((a, b) => a + (b.portfolio?.winning_trades || 0), 0),
    losses: bots.reduce((a, b) => a + (b.portfolio?.losing_trades || 0), 0),
  }

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
            <button style={{ padding: '6px 12px', fontSize: '11px', fontWeight: 700, borderRadius: '7px', background: '#3b3bff', border: 'none', color: '#fff' }}>🎮 DEMO</button>
            <button style={{ padding: '6px 12px', fontSize: '11px', fontWeight: 700, borderRadius: '7px', background: 'transparent', border: 'none', color: '#4b5563' }}>⚡ LIVE</button>
          </div>
          <button style={{ padding: '10px 20px', background: '#3b3bff', border: 'none', borderRadius: '10px', color: '#fff', fontSize: '12px', fontWeight: 600, cursor: 'pointer' }}>+ Új bot</button>
        </div>
      </div>

      {/* 2. STATS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '25px' }}>
        <SummaryCard label="Aktív botok" value={totalStats.active} color="#a3e635" />
        <SummaryCard label="Összes PnL" value={`$${totalStats.pnl.toFixed(2)}`} color={totalStats.pnl >= 0 ? "#4ade80" : "#f87171"} />
        <SummaryCard label="Összes Trade" value={totalStats.trades} color="#e2e8f0" />
        <SummaryCard label="Egyenleg Sum" value={`$${totalStats.balance.toFixed(2)}`} color="#6366f1" />
        <div style={{ background: '#13131f', padding: '15px', borderRadius: '12px', border: '1px solid #252535' }}>
          <p style={{ fontSize: '10px', color: '#4b5563', margin: '0 0 8px', fontWeight: 700, textTransform: 'uppercase' }}>Total Win / Loss</p>
          <p style={{ fontSize: '20px', fontWeight: 700, margin: 0 }}>
            <span style={{ color: '#4ade80' }}>{totalStats.wins}W</span>
            <span style={{ color: '#4b5563', margin: '0 5px' }}>/</span>
            <span style={{ color: '#f87171' }}>{totalStats.losses}L</span>
          </p>
        </div>
      </div>

      {/* 3. SZŰRŐK ÉS GYORS GOMBOK */}
      <div style={{ display: 'flex', gap: '12px', marginBottom: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ position: 'relative', flex: 1, minWidth: '200px' }}>
          <Search size={16} style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', color: '#4b5563' }} />
          <input type="text" placeholder="Bot keresése..." value={search} onChange={e => setSearch(e.target.value)} style={{ width: '100%', padding: '10px 10px 10px 38px', background: '#13131f', border: '1px solid #252535', borderRadius: '10px', color: '#fff', outline: 'none', fontSize: '13px' }} />
        </div>

        <div style={{ display: 'flex', background: '#13131f', padding: '3px', borderRadius: '10px', border: '1px solid #252535' }}>
          {(['all', 'running', 'stopped', 'error'] as const).map(f => (
            <button key={f} onClick={() => setStatusFilter(f)} style={{ padding: '7px 14px', fontSize: '11px', fontWeight: 700, borderRadius: '8px', border: 'none', background: statusFilter === f ? '#3b3bff20' : 'transparent', color: statusFilter === f ? '#818cf8' : '#4b5563', cursor: 'pointer' }}>
              {f === 'all' ? 'Összes' : f === 'running' ? `● Aktív (${totalStats.active})` : f === 'stopped' ? '■ Leállítva' : '✕ Hiba'}
            </button>
          ))}
        </div>

        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <button onClick={() => setQuickFilter(quickFilter === 'best3' ? 'none' : 'best3')} style={{ padding: '10px', borderRadius: '8px', border: '1px solid #252535', background: '#13131f', color: '#4b5563' }}><Trophy size={16} /></button>
          <button onClick={() => setQuickFilter(quickFilter === 'worst3' ? 'none' : 'worst3')} style={{ padding: '10px', borderRadius: '8px', border: '1px solid #252535', background: '#13131f', color: '#4b5563' }}><AlertTriangle size={16} /></button>
          <select value={sortKey} onChange={e => setSortKey(e.target.value as SortKey)} style={{ background: '#13131f', color: '#fff', border: '1px solid #252535', padding: '10px 15px', borderRadius: '10px', outline: 'none', fontSize: '13px', cursor: 'pointer' }}>
            <option value="pnl">Profit</option>
            <option value="winRate">Win Rate</option>
            <option value="balance">Egyenleg</option>
          </select>
          <button onClick={() => setSortDir(d => d === 'asc' ? 'desc' : 'asc')} style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '10px', padding: '10px', color: '#4b5563', cursor: 'pointer' }}><ArrowUpDown size={16} /></button>
        </div>

        <button style={{ padding: '10px 15px', background: '#22c55e15', color: '#22c55e', border: '1px solid #22c55e30', borderRadius: '10px', fontSize: '12px', fontWeight: 800 }}>▶ Indít mind</button>
        <button style={{ padding: '10px 15px', background: '#fbbf2415', color: '#fbbf24', border: '1px solid #fbbf2430', borderRadius: '10px', fontSize: '12px', fontWeight: 800 }}>■ Megállít mind</button>
        <button onClick={() => { if (confirm("Minden statisztikát nullázol?")) apiFetch("/bots/reset-all", { method: "POST" }).then(loadBots) }} style={{ padding: '10px 15px', background: '#3b3bff15', color: '#818cf8', border: '1px solid #3b3bff30', borderRadius: '10px', fontSize: '12px', fontWeight: 800 }}><RotateCcw size={14} style={{ marginRight: 8 }} /> Összes statisztika nullázása</button>
      </div>

      {/* --- 3. AKTÍV POZÍCIÓK LISTÁJA --- */}
      <div style={{ marginBottom: '25px', padding: '15px', background: 'rgba(59, 130, 246, 0.03)', borderRadius: '12px', border: '1px solid rgba(59, 130, 246, 0.15)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '12px' }}>
          <Zap size={16} color="#3b82f6" fill="#3b82f6" />
          <span style={{ fontSize: '11px', fontWeight: 800, color: '#3b82f6', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Aktív pozíciók a piacon</span>
        </div>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
          {botsInPosition.length > 0 ? (
            botsInPosition.map(bot => (
              <motion.div key={bot.id} animate={{ opacity: [1, 0.6, 1] }} transition={{ duration: 2, repeat: Infinity }} style={{ padding: '8px 16px', background: '#1e1e30', borderRadius: '10px', border: '1.5px solid #3b82f6', display: 'flex', alignItems: 'center', gap: '10px' }}>
                <span style={{ fontSize: '12px', fontWeight: 700, color: '#fff' }}>{bot.name}</span>
                <span style={{ fontSize: '9px', fontWeight: 900, color: '#3b82f6' }}>LIVE</span>
              </motion.div>
            ))
          ) : (
            <span style={{ fontSize: '11px', color: '#4b5563', fontStyle: 'italic' }}>Jelenleg nincs nyitott pozíció.</span>
          )}
        </div>
      </div>

      {/* 4. BOT RÁCS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '30px' }}>
        {filteredBots.map((bot) => (
          <BotCard
            key={bot.id}
            bot={bot}
            isInPos={checkPosition(bot)}
            isLoading={actionLoading === bot.id}
            onStart={() => apiFetch(`/bots/${bot.id}/start`, { method: "POST" }).then(loadBots)}
            onStop={() => apiFetch(`/bots/${bot.id}/stop`, { method: "POST" }).then(loadBots)}
            onReset={() => { if (confirm("Nullázás?")) apiFetch(`/bots/${bot.id}/reset`, { method: "POST" }).then(loadBots) }}
            onDelete={() => { if (confirm("Törlés?")) apiFetch(`/bots/${bot.id}`, { method: "DELETE" }).then(loadBots) }}
          />
        ))}
      </div>

      {/* 5. NAPLÓ */}
      <div style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '16px', padding: '18px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '15px', color: '#6366f1' }}>
          <ScrollText size={18} /><h2 style={{ fontSize: '13px', fontWeight: 700, margin: 0, textTransform: 'uppercase' }}>Eseménynapló</h2>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', maxHeight: '150px', overflowY: 'auto' }}>
          <p style={{ fontSize: '11px', color: '#4b5563', textAlign: 'center' }}>Még nincs esemény...</p>
        </div>
        <div style={{ marginTop: '15px', display: 'flex', justifyContent: 'flex-end', gap: '10px', fontSize: '11px', color: '#4b5563' }}>
          <span style={{ display: 'flex', alignItems: 'center', gap: '5px' }}><div style={{ width: 6, height: 6, borderRadius: '50%', background: '#22c55e' }} /> ONLINE</span>
          <span>Frissítve: {lastSync.toLocaleTimeString()}</span>
        </div>
      </div>
    </div>
  )
}

function SummaryCard({ label, value, color }: { label: string, value: string | number, color: string }) {
  return (
    <div style={{ background: '#13131f', padding: '15px', borderRadius: '12px', border: '1px solid #252535' }}>
      <p style={{ fontSize: '10px', color: '#4b5563', margin: '0 0 8px', fontWeight: 700, textTransform: 'uppercase' }}>{label}</p>
      <p style={{ fontSize: '20px', fontWeight: 700, color: color, margin: 0 }}>{value}</p>
    </div>
  )
}

function BotCard({ bot, isInPos, onStart, onStop, onReset, onDelete, isLoading }: any) {
  const pnl = bot.portfolio?.total_pnl || 0
  const balance = bot.portfolio?.balance || 0
  const strategyColor = STRATEGY_COLORS[bot.strategy_type] || '#818cf8'

  return (
    <motion.div layout style={{
      background: '#13131f',
      border: isInPos ? '1.5px solid #22c55e' : '1px solid #252535',
      borderRadius: '16px',
      padding: '15px',
      display: 'flex',
      flexDirection: 'column',
      gap: '10px',
      backgroundColor: isInPos ? 'rgba(34, 197, 94, 0.05)' : '#13131f',
      position: 'relative'
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ overflow: 'hidden' }}>
          <h3 style={{ fontSize: '13px', fontWeight: 700, margin: 0, whiteSpace: 'nowrap' }}>{bot.name}</h3>
          <span style={{ fontSize: '8px', fontWeight: 800, color: strategyColor, background: `${strategyColor}15`, padding: '2px 6px', borderRadius: '4px', marginTop: '4px', display: 'inline-block' }}>{bot.strategy_type.toUpperCase()}</span>
        </div>
        <div style={{ width: '7px', height: '7px', borderRadius: '50%', background: bot.status === 'running' ? '#22c55e' : '#4b5563' }} />
      </div>

      <div style={{ background: '#0d0d1a', padding: '10px', borderRadius: '12px', border: '1px solid #1e1e30' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '5px' }}>
          <span style={{ fontSize: '8px', color: '#4b5563', fontWeight: 700 }}>EGYENLEG</span>
          <span style={{ fontSize: '12px', fontWeight: 700 }}>${balance.toFixed(2)}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <span style={{ fontSize: '8px', color: '#4b5563', fontWeight: 700 }}>PnL</span>
          <span style={{ fontSize: '14px', fontWeight: 800, color: pnl >= 0 ? '#4ade80' : '#f87171' }}>{pnl >= 0 ? '+' : ''}${pnl.toFixed(2)}</span>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '5px' }}>
        <MiniStat label="Tét" value={`$${bot.bet_size}`} />
        <MiniStat label="SL" value="-10%" />
        <MiniStat label="TP" value="+20%" />
      </div>

      <div style={{ marginTop: '5px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '8px', color: '#4b5563', marginBottom: '3px', fontWeight: 700 }}>
          <span>UTÓBBI KÖTÉSEK ({bot.portfolio?.winning_trades || 0}W / {bot.portfolio?.losing_trades || 0}L)</span>
        </div>
        <div style={{ height: '40px', background: '#080812', borderRadius: '8px', border: '1px solid #1e1e30', padding: '5px', overflowY: 'auto' }}>
          <p style={{ fontSize: '8px', color: '#333', textAlign: 'center', marginTop: '10px' }}>Még nincs kötés...</p>
        </div>
      </div>

      <div style={{ height: '3px', background: '#1e1e30', borderRadius: '2px', overflow: 'hidden' }}>
        <div style={{ height: '100%', width: `${bot.portfolio?.win_rate || 0}%`, background: '#22c55e' }} />
      </div>

      <div style={{ display: 'flex', gap: '5px' }}>
        <button onClick={bot.status === 'running' ? onStop : onStart} style={{ flex: 3, padding: '10px', background: bot.status === 'running' ? '#fbbf2415' : '#22c55e15', color: bot.status === 'running' ? '#fbbf24' : '#22c55e', border: 'none', borderRadius: '10px', cursor: 'pointer', fontSize: '10px', fontWeight: 800 }}>
          {isLoading ? '...' : (bot.status === 'running' ? 'LEÁLLÍT' : 'INDÍTÁS')}
        </button>
        <button onClick={onReset} style={{ flex: 1, padding: '10px', background: '#3b3bff15', color: '#818cf8', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><RotateCcw size={14} /></button>
        <button onClick={onDelete} style={{ flex: 1, padding: '10px', background: '#ef444415', color: '#ef4444', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><Trash2 size={14} /></button>
      </div>
    </motion.div>
  )
}

function MiniStat({ label, value }: any) {
  return (
    <div style={{ background: '#1a1a2e', padding: '4px', borderRadius: '6px', textAlign: 'center' }}>
      <p style={{ fontSize: '6px', color: '#4b5563', margin: 0, textTransform: 'uppercase' }}>{label}</p>
      <p style={{ fontSize: '10px', color: '#e2e8f0', fontWeight: 700, margin: 0 }}>{value}</p>
    </div>
  )
}