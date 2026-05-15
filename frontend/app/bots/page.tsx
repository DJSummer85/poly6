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
            <button style={{ padding: '6px 12px', fontSize: '11px', fontWeight: 700, borderRadius: '7px', background: '#3b3bff', border: 'none', color: '#fff' }}>🎮 DEMO</button>
            <button style={{ padding: '6px 12px', fontSize: '11px', fontWeight: 700, borderRadius: '7px', background: 'transparent', border: 'none', color: '#4b5563' }}>⚡ LIVE</button>
          </div>
          <button style={{ padding: '10px 20px', background: '#3b3bff', border: 'none', borderRadius: '10px', color: '#fff', fontSize: '12px', fontWeight: 600, cursor: 'pointer' }}>+ Új bot</button>
        </div>
      </div>

      {/* 2. STATS */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '25px' }}>
        <SummaryCard label="Aktív botok" value={bots.filter(b => b.status === 'running').length} color="#a3e635" />
        <SummaryCard label="Összes PnL" value={`$${bots.reduce((a, b) => a + (b.portfolio?.total_pnl || 0), 0).toFixed(2)}`} color="#f87171" />
        <SummaryCard label="Összes Trade" value={bots.reduce((a, b) => a + (b.portfolio?.total_trades || 0), 0)} color="#e2e8f0" />
        <SummaryCard label="Egyenleg Sum" value={`$${bots.reduce((a, b) => a + (b.portfolio?.balance || 0), 0).toFixed(2)}`} color="#6366f1" />
        <div style={{ background: '#13131f', padding: '15px', borderRadius: '12px', border: '1px solid #252535' }}>
          <p style={{ fontSize: '10px', color: '#4b5563', margin: '0 0 8px', fontWeight: 700, textTransform: 'uppercase' }}>Total Win / Loss</p>
          <p style={{ fontSize: '20px', fontWeight: 700, margin: 0 }}>
            <span style={{ color: '#4ade80' }}>{bots.reduce((a, b) => a + (b.portfolio?.winning_trades || 0), 0)}W</span>
            <span style={{ color: '#4b5563', margin: '0 5px' }}>/</span>
            <span style={{ color: '#f87171' }}>{bots.reduce((a, b) => a + (b.portfolio?.losing_trades || 0), 0)}L</span>
          </p>
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

        <button onClick={() => handleBulk("start-all")} style={{ padding: '8px 15px', background: '#22c55e15', color: '#22c55e', border: '1px solid #22c55e30', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}>▶ Indít mind</button>
        <button onClick={() => handleBulk("stop-all")} style={{ padding: '8px 15px', background: '#fbbf2415', color: '#fbbf24', border: '1px solid #fbbf2430', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}>■ Megállít mind</button>
        <button onClick={() => { if (confirm("Mindent nullázol?")) handleBulk("reset-all") }} style={{ padding: '8px 15px', background: '#3b3bff15', color: '#818cf8', border: '1px solid #3b3bff30', borderRadius: '8px', fontSize: '11px', fontWeight: 800 }}><RotateCcw size={14} style={{ marginRight: 8 }} /> Összes statisztika nullázása</button>
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
          />
        ))}
      </div>

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
    </div>
  )
}

function BotCard({ bot, isInPos, onAction, onDelete, isLoading }: any) {
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
      {isInPos && <div style={{ position: 'absolute', top: '10px', right: '10px', background: '#22c55e', color: '#000', fontSize: '8px', fontWeight: 900, padding: '2px 6px', borderRadius: '4px' }}>LIVE</div>}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h3 style={{ fontSize: '13px', fontWeight: 700, margin: 0 }}>{bot.name}</h3>
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

      <div style={{ marginTop: '5px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '8px', color: '#4b5563', marginBottom: '3px', fontWeight: 700 }}><span>UTÓBBI KÖTÉSEK ({bot.portfolio?.winning_trades || 0}W / {bot.portfolio?.losing_trades || 0}L)</span></div>
        <div style={{ height: '40px', background: '#080812', borderRadius: '8px', border: '1px solid #1e1e30', padding: '5px', overflowY: 'auto' }}>
          {bot.history?.map((t: any) => (<div key={t.id} style={{ display: 'flex', justifyContent: 'space-between', fontSize: '8px', marginBottom: 2 }}><span style={{ color: t.win ? '#22c55e' : '#ef4444' }}>{t.win ? '✅ NYERT' : '❌ VESZTETT'}</span><span>${t.amount.toFixed(2)}</span></div>))}
          {(!bot.history || bot.history.length === 0) && <p style={{ fontSize: '8px', color: '#333', textAlign: 'center', marginTop: '10px' }}>Még nincs kötés...</p>}
        </div>
      </div>

      <div style={{ height: '3px', background: '#1e1e30', borderRadius: '2px', overflow: 'hidden' }}><div style={{ height: '100%', width: `${bot.portfolio?.win_rate || 0}%`, background: '#22c55e' }} /></div>

      <div style={{ display: 'flex', gap: '5px' }}>
        <button onClick={() => onAction(bot.status === 'running' ? 'stop' : 'start')} disabled={isLoading} style={{ flex: 3, padding: '10px', background: bot.status === 'running' ? '#fbbf2415' : '#22c55e15', color: bot.status === 'running' ? '#fbbf24' : '#22c55e', border: 'none', borderRadius: '10px', cursor: 'pointer', fontSize: '10px', fontWeight: 800 }}>{isLoading ? '...' : (bot.status === 'running' ? 'LEÁLLÍT' : 'INDÍTÁS')}</button>
        <button onClick={() => onAction('reset')} style={{ flex: 1, padding: '10px', background: '#3b3bff15', color: '#818cf8', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><RotateCcw size={14} /></button>
        <button onClick={onDelete} style={{ flex: 1, padding: '10px', background: '#ef444415', color: '#ef4444', border: 'none', borderRadius: '10px', cursor: 'pointer' }}><Trash2 size={14} /></button>
      </div>
    </motion.div>
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
