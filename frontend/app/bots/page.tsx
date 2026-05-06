'use client'

import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Activity, Bot as BotIcon, Loader2, Play, Plus, Square, Trash2, RotateCcw,
  Shield, Target, Wallet, Search, ArrowUpDown, Wifi, WifiOff, Trophy, AlertTriangle,
  X, TrendingUp, ScrollText, Clock, History
} from "lucide-react"
import { toast } from "sonner"
import { apiFetch } from "@/lib/utils"
import { useAppStore } from "@/store"

// ---- Típusok ----
type BotStatus = 'running' | 'paused' | 'error' | 'stopped'
type SortKey = 'pnl' | 'winRate' | 'trades' | 'balance' | 'name'

interface TradeResult {
  id: string
  win: boolean
  amount: number
  time: string
}

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
  history?: TradeResult[] // ÚJ: Egyedi trade napló a botnak
  portfolio?: {
    balance: number
    total_pnl: number
    total_trades: number
    winning_trades: number
    losing_trades: number
    win_rate: number
  }
}

interface LogEntry {
  id: string
  time: string
  msg: string
  type: 'info' | 'success' | 'warn' | 'error'
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
  const [logs, setLogs] = useState<LogEntry[]>([])
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

  const addLog = useCallback((msg: string, type: LogEntry['type'] = 'info') => {
    const newEntry = { id: Math.random().toString(), time: new Date().toLocaleTimeString(), msg, type }
    setLogs(prev => [newEntry, ...prev].slice(0, 20))
  }, [])

  // ---- Adatok betöltése és Trade Naplózás ----
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

      // ÚJ TRADE ÉSZLELÉSE ÉS KÁRTYA-LOG FRISSÍTÉSE
      if (prevBotsRef.current.length > 0) {
        withPortfolio.forEach((newBot, index) => {
          const oldBot = prevBotsRef.current.find(b => b.id === newBot.id)
          if (oldBot?.portfolio && newBot.portfolio) {
            if (newBot.portfolio.total_trades > oldBot.portfolio.total_trades) {
              const pnlDiff = newBot.portfolio.total_pnl - oldBot.portfolio.total_pnl
              const isWin = pnlDiff >= 0

              // Hozzáadjuk a bot saját history-jához a memóriában
              const newTrade: TradeResult = {
                id: Math.random().toString(),
                win: isWin,
                amount: Math.abs(pnlDiff),
                time: new Date().toLocaleTimeString()
              }

              // Megkeressük és frissítjük a konkrét botot a listában a history-val
              newBot.history = [newTrade, ...(oldBot.history || [])].slice(0, 5)

              addLog(`${newBot.name}: ${isWin ? 'NYERTES' : 'VESZTES'} trade lezárva! ($${newTrade.amount.toFixed(2)})`, isWin ? 'success' : 'warn')
            } else {
              // Megtartjuk a meglévő history-t a frissítés után is
              newBot.history = oldBot.history
            }
          }
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

  // ---- Műveletek ----
  const handleStart = async (id: string, name: string) => {
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/start`, { method: "POST" })
      toast.success(`${name} elindítva`)
      addLog(`${name}: Kézi indítás.`, "success")
      await loadBots()
    } catch (err: any) { toast.error(err.message) }
    finally { setActionLoading(null) }
  }

  const handleStop = async (id: string, name: string) => {
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/stop`, { method: "POST" })
      toast.success(`${name} leállítva`)
      addLog(`${name}: Kézi leállítás.`, "warn")
      await loadBots()
    } catch (err: any) { toast.error(err.message) }
    finally { setActionLoading(null) }
  }

  const handleReset = async (id: string, name: string) => {
    if (!confirm(`Resetelsz minden értéket: ${name}?`)) return
    setActionLoading(id)
    try {
      await apiFetch(`/bots/${id}/reset`, { method: "POST" })
      toast.success("Nullázva")
      addLog(`${name}: Statisztikák nullázva.`, "info")
      // Ürítjük a history-t is
      setBots(prev => prev.map(b => b.id === id ? { ...b, history: [] } : b))
      await loadBots()
    } catch { toast.error("Backend hiba") }
    finally { setActionLoading(null) }
  }

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
          <div>
            <h1 style={{ fontSize: '22px', fontWeight: 700, margin: 0 }}>Trading Dashboard</h1>
            <p style={{ color: '#4b5563', fontSize: '13px', margin: 0 }}>Advanced Bot Management</p>
          </div>
        </div>
        <div style={{ display: 'flex', gap: '10px' }}>
          <button style={{ padding: '10px 20px', background: '#3b3bff', border: 'none', borderRadius: '10px', color: '#fff', fontSize: '12px', fontWeight: 600, cursor: 'pointer' }}>+ Új bot</button>
        </div>
      </div>

      {/* 2. STATISZTIKAI KÁRTYÁK */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '25px' }}>
        <SummaryCard label="Aktív botok" value={totalStats.active} color="#a3e635" />
        <SummaryCard label="Teljes PnL" value={`$${totalStats.pnl.toFixed(2)}`} color={totalStats.pnl >= 0 ? "#4ade80" : "#f87171"} />
        <SummaryCard label="Összes Trade" value={totalStats.trades} color="#e2e8f0" />
        <SummaryCard label="Egyenleg Sum" value={`$${totalStats.balance.toFixed(2)}`} color="#6366f1" />
        <div style={{ background: '#13131f', padding: '15px', borderRadius: '12px', border: '1px solid #252535' }}>
          <p style={{ fontSize: '11px', color: '#4b5563', margin: '0 0 8px' }}>Total Win / Loss</p>
          <p style={{ fontSize: '20px', fontWeight: 700, margin: 0 }}>
            <span style={{ color: '#4ade80' }}>{totalStats.wins}W</span>
            <span style={{ color: '#4b5563', margin: '0 5px' }}>/</span>
            <span style={{ color: '#f87171' }}>{totalStats.losses}L</span>
          </p>
        </div>
      </div>

      {/* 3. SZŰRŐK ÉS RENDEZÉS */}
      <div style={{ display: 'flex', gap: '12px', marginBottom: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ position: 'relative', flex: 1, minWidth: '200px' }}>
          <Search size={16} style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)', color: '#4b5563' }} />
          <input type="text" placeholder="Keresés..." value={search} onChange={e => setSearch(e.target.value)} style={{ width: '100%', padding: '10px 10px 10px 38px', background: '#13131f', border: '1px solid #252535', borderRadius: '10px', color: '#fff', outline: 'none', fontSize: '13px' }} />
        </div>
        <select value={sortKey} onChange={e => setSortKey(e.target.value as SortKey)} style={{ background: '#13131f', color: '#fff', border: '1px solid #252535', padding: '10px 15px', borderRadius: '10px', outline: 'none', fontSize: '13px', cursor: 'pointer' }}>
          <option value="pnl">Legtöbb profit</option>
          <option value="winRate">Win Rate</option>
          <option value="balance">Egyenleg</option>
          <option value="name">Név</option>
        </select>
        <button onClick={() => setSortDir(d => d === 'asc' ? 'desc' : 'asc')} style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '10px', padding: '10px', color: '#4b5563', cursor: 'pointer' }}><ArrowUpDown size={16} /></button>
      </div>

      {/* 4. GYORS SZŰRŐK */}
      <div style={{ display: 'flex', gap: '10px', marginBottom: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
        <button onClick={() => setQuickFilter(quickFilter === 'best3' ? 'none' : 'best3')} style={{ display: 'flex', alignItems: 'center', gap: '6px', padding: '7px 15px', borderRadius: '8px', border: '1px solid #252535', background: quickFilter === 'best3' ? '#fbbf2415' : '#13131f', color: quickFilter === 'best3' ? '#fbbf24' : '#6b7280', fontSize: '12px', cursor: 'pointer' }}><Trophy size={14} /> Top 3 Legjobb</button>
        <button onClick={() => setQuickFilter(quickFilter === 'worst3' ? 'none' : 'worst3')} style={{ display: 'flex', alignItems: 'center', gap: '6px', padding: '7px 15px', borderRadius: '8px', border: '1px solid #252535', background: quickFilter === 'worst3' ? '#ef444415' : '#13131f', color: quickFilter === 'worst3' ? '#ef4444' : '#6b7280', fontSize: '12px', cursor: 'pointer' }}><AlertTriangle size={14} /> Top 3 Legrosszabb</button>
      </div>

      {/* 5. BOT RÁCS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '30px' }}>
        {filteredBots.map((bot) => (
          <BotCard
            key={bot.id}
            bot={bot}
            isLoading={actionLoading === bot.id}
            onStart={() => handleStart(bot.id, bot.name)}
            onStop={() => handleStop(bot.id, bot.name)}
            onReset={() => handleReset(bot.id, bot.name)}
            onDelete={() => apiFetch(`/bots/${bot.id}`, { method: "DELETE" }).then(loadBots)}
          />
        ))}
      </div>

      {/* 6. GLOBAL ACTIVITY LOG */}
      <div style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '16px', padding: '18px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '15px', color: '#6366f1' }}>
          <ScrollText size={18} />
          <h2 style={{ fontSize: '14px', fontWeight: 700, margin: 0, textTransform: 'uppercase', letterSpacing: '0.05em' }}>Eseménynapló</h2>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', maxHeight: '180px', overflowY: 'auto' }}>
          {logs.map(log => (
            <div key={log.id} style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', padding: '8px 12px', background: '#0b0b14', borderRadius: '8px', borderLeft: `3px solid ${log.type === 'success' ? '#22c55e' : log.type === 'warn' ? '#fbbf24' : '#6366f1'}` }}>
              <span>{log.msg}</span>
              <span style={{ color: '#4b5563' }}>{log.time}</span>
            </div>
          ))}
        </div>
      </div>

      <div style={{ marginTop: '20px', display: 'flex', alignItems: 'center', gap: '10px', fontSize: '11px', color: '#4b5563' }}>
        {serverOnline ? <Wifi size={14} color="#22c55e" /> : <WifiOff size={14} color="#ef4444" />}
        <span>Frissítve: {lastSync.toLocaleTimeString()}</span>
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
  const wins = bot.portfolio?.winning_trades || 0
  const losses = bot.portfolio?.losing_trades || 0
  const strategyColor = STRATEGY_COLORS[bot.strategy_type] || '#818cf8'

  return (
    <motion.div layout style={{ background: '#13131f', border: '1px solid #252535', borderRadius: '16px', padding: '15px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
      {/* Fejléc */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ overflow: 'hidden' }}>
          <h3 style={{ fontSize: '14px', fontWeight: 700, margin: 0, whiteSpace: 'nowrap', textOverflow: 'ellipsis' }}>{bot.name}</h3>
          <span style={{ fontSize: '9px', fontWeight: 800, color: strategyColor, background: `${strategyColor}15`, padding: '2px 6px', borderRadius: '4px', marginTop: '4px', display: 'inline-block' }}>{bot.strategy_type.toUpperCase()}</span>
        </div>
        <div style={{ width: '8px', height: '8px', borderRadius: '50%', background: bot.status === 'running' ? '#22c55e' : '#4b5563', boxShadow: bot.status === 'running' ? '0 0 10px #22c55e' : 'none' }} />
      </div>

      {/* Egyenleg és Profit */}
      <div style={{ background: '#0d0d1a', padding: '10px', borderRadius: '12px', border: '1px solid #1e1e30' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '5px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <Wallet size={12} color="#6366f1" />
            <span style={{ fontSize: '9px', color: '#4b5563', fontWeight: 600 }}>EGYENLEG</span>
          </div>
          <span style={{ fontSize: '13px', fontWeight: 700 }}>${balance.toFixed(2)}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <TrendingUp size={12} color={pnl >= 0 ? '#4ade80' : '#f87171'} />
            <span style={{ fontSize: '9px', color: '#4b5563', fontWeight: 600 }}>PnL</span>
          </div>
          <span style={{ fontSize: '15px', fontWeight: 800, color: pnl >= 0 ? '#4ade80' : '#f87171' }}>{pnl >= 0 ? '+' : ''}${pnl.toFixed(2)}</span>
        </div>
      </div>

      {/* Config & Win Rate */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '5px' }}>
        <div style={{ background: '#1a1a2e', padding: '5px', borderRadius: '8px', textAlign: 'center' }}>
          <p style={{ fontSize: '7px', color: '#4b5563', margin: 0 }}>TÉT</p>
          <p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>${bot.bet_size}</p>
        </div>
        <div style={{ background: '#1a1a2e', padding: '5px', borderRadius: '8px', textAlign: 'center' }}>
          <p style={{ fontSize: '7px', color: '#ef4444', margin: 0 }}>SL</p>
          <p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>-{(bot.stop_loss * 100).toFixed(0)}%</p>
        </div>
        <div style={{ background: '#1a1a2e', padding: '5px', borderRadius: '8px', textAlign: 'center' }}>
          <p style={{ fontSize: '7px', color: '#22c55e', margin: 0 }}>TP</p>
          <p style={{ fontSize: '10px', fontWeight: 700, margin: 0 }}>+{(bot.take_profit * 100).toFixed(0)}%</p>
        </div>
      </div>

      {/* BOT LOG ABLAK - ÚJ: Utolsó 5 kötés */}
      <div style={{ background: '#080812', borderRadius: '10px', padding: '8px', border: '1px solid #1a1a2e' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '5px', marginBottom: '5px' }}>
          <History size={10} color="#4b5563" />
          <span style={{ fontSize: '9px', fontWeight: 700, color: '#4b5563' }}>UTÓBBI KÖTÉSEK ({wins}W / {losses}L)</span>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', height: '65px', overflowY: 'auto' }}>
          {bot.history && bot.history.length > 0 ? (
            bot.history.map(t => (
              <div key={t.id} style={{ display: 'flex', justifyContent: 'space-between', fontSize: '9px', background: '#13131f', padding: '3px 6px', borderRadius: '4px' }}>
                <span style={{ color: t.win ? '#22c55e' : '#ef4444', fontWeight: 800 }}>{t.win ? '✅ NYERT' : '❌ VESZTETT'}</span>
                <span style={{ fontWeight: 700 }}>${t.amount.toFixed(2)}</span>
              </div>
            ))
          ) : (
            <p style={{ fontSize: '8px', color: '#333', textAlign: 'center', marginTop: '15px' }}>Még nincs kötés...</p>
          )}
        </div>
      </div>

      {/* Win Rate Bar */}
      <div style={{ height: '4px', background: '#1e1e30', borderRadius: '2px', overflow: 'hidden' }}>
        <div style={{ height: '100%', width: `${bot.portfolio?.win_rate || 0}%`, background: '#22c55e', transition: 'width 1s ease' }} />
      </div>

      {/* Gombok */}
      <div style={{ display: 'flex', gap: '5px', marginTop: '5px' }}>
        {bot.status === 'running' ? (
          <button onClick={onStop} disabled={isLoading} style={{ flex: 3, padding: '10px', background: '#fbbf2415', color: '#fbbf24', border: '1px solid #fbbf2430', borderRadius: '10px', cursor: 'pointer', fontSize: '11px', fontWeight: 800 }}>
            {isLoading ? <Loader2 size={14} className="animate-spin" /> : 'LEÁLLÍT'}
          </button>
        ) : (
          <button onClick={onStart} disabled={isLoading} style={{ flex: 3, padding: '10px', background: '#22c55e15', color: '#22c55e', border: '1px solid #22c55e30', borderRadius: '10px', cursor: 'pointer', fontSize: '11px', fontWeight: 800 }}>
            {isLoading ? <Loader2 size={14} className="animate-spin" /> : 'INDÍTÁS'}
          </button>
        )}
        <button onClick={onReset} disabled={isLoading} style={{ flex: 1, padding: '10px', background: '#3b3bff15', color: '#818cf8', border: '1px solid #3b3bff30', borderRadius: '10px', cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}><RotateCcw size={16} /></button>
        <button onClick={onDelete} style={{ flex: 1, padding: '10px', background: '#ef444415', color: '#ef4444', border: '1px solid #ef444430', borderRadius: '10px', cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}><Trash2 size={16} /></button>
      </div>
    </motion.div>
  )
}