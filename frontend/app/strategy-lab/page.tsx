"use client";

import { motion } from "framer-motion";
import {
  Activity,
  Beaker,
  Clock,
  FlaskConical,
  Loader2,
  Play,
  TrendingDown,
  TrendingUp,
  X,
  Zap,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { EmptyState } from "@/components/ui/empty-state";
import { ErrorState } from "@/components/ui/error-state";
import { GlassCard } from "@/components/ui/glass-card";
import { SkeletonCard } from "@/components/ui/skeleton-card";
import { StatCard } from "@/components/ui/stat-card";
import { apiFetch, formatPercent, formatPrice, formatTime } from "@/lib/utils";

type StrategyInfo = {
  id: string;
  name: string;
  description: string;
  params: string[];
};

type StrategyTest = {
  id: number;
  strategy_type: string;
  market_id: string;
  status: string;
  initial_balance: number;
  final_balance: number;
  total_trades: number;
  winning_trades: number;
  losing_trades: number;
  total_pnl: number;
};

type TestSignal = {
  side: string;
  confidence: number;
  reason: string;
};

type TestResult = {
  id: number;
  strategy_type: string;
  market_id: string;
  status: string;
  initial_balance: number;
  final_balance: number;
  signal: TestSignal;
  total_trades: number;
  total_pnl: number;
};

type Performance = {
  initial_balance: number;
  final_balance: number;
  total_trades: number;
  winning_trades: number;
  losing_trades: number;
  win_rate: number;
  total_pnl: number;
  roi: number;
};

type TestEvent = {
  intents: Array<{
    id: number;
    side: string;
    strategy_type: string;
    confidence: number;
    reason: string;
    status: string;
    snapshot_json: string;
    created_at: string;
  }>;
  executions: Array<{
    id: number;
    side: string;
    status: string;
    filled_size: number;
    avg_fill_price: number;
    error_code: string | null;
    created_at: string;
  }>;
};

const STRATEGY_COLORS: Record<string, string> = {
  momentum: "#8b5cf6",
  mean_reversion: "#06b6d4",
  trend: "#10b981",
  volatility: "#f59e0b",
  sniper: "#ef4444",
  contrarian: "#ec4899",
  binance_velocity: "#6366f1",
  fair_value: "#14b8a6",
  oracle_lag: "#a855f7",
  window_delta: "#3b82f6",
};

export default function StrategyLabPage() {
  const [strategies, setStrategies] = useState<StrategyInfo[]>([]);
  const [tests, setTests] = useState<StrategyTest[]>([]);
  const [selectedTest, setSelectedTest] = useState<StrategyTest | null>(null);
  const [testResult, setTestResult] = useState<TestResult | null>(null);
  const [performance, setPerformance] = useState<Performance | null>(null);
  const [testEvents, setTestEvents] = useState<TestEvent | null>(null);

  const [loadingStrategies, setLoadingStrategies] = useState(true);
  const [loadingTests, setLoadingTests] = useState(true);
  const [runningTest, setRunningTest] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [selectedStrategy, setSelectedStrategy] = useState<string>("momentum");
  const [marketId, setMarketId] = useState<string>("");
  const [initialBalance, setInitialBalance] = useState<number>(100);

  const fetchStrategies = useCallback(async () => {
    try {
      const data = await apiFetch<{ strategies: StrategyInfo[] }>("/strategies");
      setStrategies(data.strategies);
    } catch (e) {
      console.error("Failed to fetch strategies:", e);
    } finally {
      setLoadingStrategies(false);
    }
  }, []);

  const fetchTests = useCallback(async () => {
    try {
      setTests([]);
    } catch (e) {
      console.error("Failed to fetch tests:", e);
    } finally {
      setLoadingTests(false);
    }
  }, []);

  useEffect(() => {
    fetchStrategies();
    fetchTests();
  }, [fetchStrategies, fetchTests]);

  const runTest = async () => {
    if (!marketId.trim()) {
      toast.error("Market ID is required");
      return;
    }

    setRunningTest(true);
    setError(null);
    setTestResult(null);
    setPerformance(null);
    setTestEvents(null);

    try {
      const result = await apiFetch<TestResult>("/strategy-tests", {
        method: "POST",
        body: JSON.stringify({
          strategy_type: selectedStrategy,
          market_id: marketId,
          initial_balance: initialBalance,
          mode: "demo",
        }),
      });

      setTestResult(result);
      toast.success("Test completed: " + result.signal.side + " signal");

      try {
        const perf = await apiFetch<Performance>("/strategy-tests/" + result.id + "/performance");
        setPerformance(perf);
      } catch (e) {
        console.error("Failed to fetch performance:", e);
      }

      try {
        const events = await apiFetch<TestEvent>("/strategy-tests/" + result.id + "/events");
        setTestEvents(events);
      } catch (e) {
        console.error("Failed to fetch events:", e);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Failed to run test";
      setError(msg);
      toast.error(msg);
    } finally {
      setRunningTest(false);
    }
  };

  const getStrategyColor = (id: string) => STRATEGY_COLORS[id] || "#6366f1";

  const getSignalIcon = (side: string) => {
    if (side === "YES") return <TrendingUp className="h-4 w-4 text-emerald-400" />;
    if (side === "NO") return <TrendingDown className="h-4 w-4 text-red-400" />;
    return <Activity className="h-4 w-4 text-zinc-400" />;
  };

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="max-w-7xl mx-auto space-y-6">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-violet-500/15">
              <FlaskConical className="h-5 w-5 text-violet-500" />
            </div>
            <div>
              <h1 className="text-2xl font-bold text-zinc-100">Strategy Lab</h1>
              <p className="text-sm text-zinc-500">Test strategies against live market data</p>
            </div>
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Left Column */}
          <div className="lg:col-span-1 space-y-6">
            {/* Strategy Selector */}
            <GlassCard className="p-5">
              <h2 className="text-sm font-semibold text-zinc-300 mb-4 flex items-center gap-2">
                <Beaker className="h-4 w-4 text-violet-500" />
                Select Strategy
              </h2>

              {loadingStrategies ? (
                <div className="space-y-2">
                  {[1, 2, 3].map((i) => (
                    <SkeletonCard key={i} className="h-12" />
                  ))}
                </div>
              ) : (
                <div className="space-y-2">
                  {strategies.map((strategy) => (
                    <button
                      key={strategy.id}
                      onClick={() => setSelectedStrategy(strategy.id)}
                      className={
                        "w-full text-left px-4 py-3 rounded-xl border transition-all " +
                        (selectedStrategy === strategy.id
                          ? "border-violet-500/50 bg-violet-500/10"
                          : "border-white/8 bg-white/5 hover:bg-white/8")
                      }
                    >
                      <div className="flex items-center justify-between">
                        <span
                          className="font-medium"
                          style={{ color: getStrategyColor(strategy.id) }}
                        >
                          {strategy.name}
                        </span>
                      </div>
                      <p className="text-xs text-zinc-500 mt-1">{strategy.description}</p>
                    </button>
                  ))}
                </div>
              )}
            </GlassCard>

            {/* Run Test Form */}
            <GlassCard className="p-5">
              <h2 className="text-sm font-semibold text-zinc-300 mb-4 flex items-center gap-2">
                <Play className="h-4 w-4 text-emerald-500" />
                Run Test
              </h2>

              <div className="space-y-4">
                <div>
                  <label className="block text-xs text-zinc-500 mb-1.5">Market ID</label>
                  <input
                    type="text"
                    value={marketId}
                    onChange={(e) => setMarketId(e.target.value)}
                    placeholder="e.g., bitcoin_up_down_100k"
                    className="w-full px-3 py-2 rounded-lg bg-white/5 border border-white/10 text-zinc-200 text-sm placeholder:text-zinc-600 focus:outline-none focus:border-violet-500/50"
                  />
                </div>

                <div>
                  <label className="block text-xs text-zinc-500 mb-1.5">Initial Balance ($)</label>
                  <input
                    type="number"
                    value={initialBalance}
                    onChange={(e) => setInitialBalance(Number(e.target.value))}
                    min={1}
                    max={10000}
                    className="w-full px-3 py-2 rounded-lg bg-white/5 border border-white/10 text-zinc-200 text-sm focus:outline-none focus:border-violet-500/50"
                  />
                </div>

                <div className="pt-2">
                  <div className="flex items-center justify-between text-xs text-zinc-500 mb-3">
                    <span>Selected: </span>
                    <span className="text-violet-400 font-medium">
                      {strategies.find((s) => s.id === selectedStrategy)?.name || selectedStrategy}
                    </span>
                  </div>

                  <button
                    onClick={runTest}
                    disabled={runningTest || !marketId.trim()}
                    className={
                      "w-full py-2.5 px-4 rounded-xl text-sm font-medium transition-all flex items-center justify-center gap-2 " +
                      (runningTest || !marketId.trim()
                        ? "bg-emerald-500/10 border border-emerald-500/20 text-emerald-400/50 cursor-not-allowed"
                        : "bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 hover:bg-emerald-500/20")
                    }
                  >
                    {runningTest ? (
                      <>
                        <Loader2 className="h-4 w-4 animate-spin" />
                        Running...
                      </>
                    ) : (
                      <>
                        <Zap className="h-4 w-4" />
                        Run Test Now
                      </>
                    )}
                  </button>
                </div>
              </div>
            </GlassCard>

            {/* Recent Tests */}
            <GlassCard className="p-5">
              <h2 className="text-sm font-semibold text-zinc-300 mb-4 flex items-center gap-2">
                <Clock className="h-4 w-4 text-zinc-400" />
                Recent Tests
              </h2>

              {loadingTests ? (
                <div className="space-y-2">
                  {[1, 2].map((i) => (
                    <SkeletonCard key={i} className="h-16" />
                  ))}
                </div>
              ) : tests.length === 0 ? (
                <div className="py-6">
                  <EmptyState title="No tests yet" description="Run a test to see results here" />
                </div>
              ) : (
                <div className="space-y-2">
                  {tests.map((test) => (
                    <button
                      key={test.id}
                      onClick={() => setSelectedTest(test)}
                      className={
                        "w-full text-left px-3 py-2.5 rounded-lg border transition-all " +
                        (selectedTest?.id === test.id
                          ? "border-violet-500/30 bg-violet-500/5"
                          : "border-white/8 bg-white/5 hover:bg-white/8")
                      }
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-sm text-zinc-300">{test.strategy_type}</span>
                        <span
                          className={
                            "text-xs font-medium " +
                            (test.total_pnl >= 0 ? "text-emerald-400" : "text-red-400")
                          }
                        >
                          {formatPrice(test.total_pnl)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 mt-1">
                        <span className="text-xs text-zinc-500">{test.market_id}</span>
                        <span className="text-xs text-zinc-600">-</span>
                        <span className="text-xs text-zinc-500">{test.total_trades} trades</span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </GlassCard>
          </div>

          {/* Right Column: Results */}
          <div className="lg:col-span-2 space-y-6">
            {error && <ErrorState title="Test Failed" description={error} onRetry={runTest} />}

            {testResult && (
              <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }}>
                <GlassCard className="p-5">
                  <div className="flex items-center justify-between mb-4">
                    <h2 className="text-sm font-semibold text-zinc-300 flex items-center gap-2">
                      <Zap className="h-4 w-4 text-amber-500" />
                      Latest Test Result
                    </h2>
                    <button
                      onClick={() => setTestResult(null)}
                      className="p-1.5 rounded-lg hover:bg-white/10 text-zinc-500 hover:text-zinc-300 transition-colors"
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </div>

                  <div
                    className={
                      "p-4 rounded-xl border mb-4 " +
                      (testResult.signal.side === "YES"
                        ? "bg-emerald-500/10 border-emerald-500/30"
                        : testResult.signal.side === "NO"
                          ? "bg-red-500/10 border-red-500/30"
                          : "bg-zinc-500/10 border-zinc-500/30")
                    }
                  >
                    <div className="flex items-center gap-3">
                      {getSignalIcon(testResult.signal.side)}
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="text-lg font-bold text-zinc-100">
                            {testResult.signal.side}
                          </span>
                          <span className="text-sm text-zinc-400">
                            {(testResult.signal.confidence * 100).toFixed(0)}% confidence
                          </span>
                        </div>
                        <p className="text-sm text-zinc-500 mt-0.5">{testResult.signal.reason}</p>
                      </div>
                    </div>
                  </div>

                  <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                    <div className="p-3 rounded-lg bg-white/5">
                      <p className="text-xs text-zinc-500 mb-1">Strategy</p>
                      <p className="text-sm font-medium text-zinc-200">
                        {testResult.strategy_type}
                      </p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5">
                      <p className="text-xs text-zinc-500 mb-1">Market</p>
                      <p className="text-sm font-medium text-zinc-200 truncate">
                        {testResult.market_id}
                      </p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5">
                      <p className="text-xs text-zinc-500 mb-1">Initial</p>
                      <p className="text-sm font-medium text-zinc-200">
                        {formatPrice(testResult.initial_balance)}
                      </p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5">
                      <p className="text-xs text-zinc-500 mb-1">Final</p>
                      <p
                        className={
                          "text-sm font-medium " +
                          (testResult.final_balance >= testResult.initial_balance
                            ? "text-emerald-400"
                            : "text-red-400")
                        }
                      >
                        {formatPrice(testResult.final_balance)}
                      </p>
                    </div>
                  </div>
                </GlassCard>
              </motion.div>
            )}

            {performance && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
              >
                <GlassCard className="p-5">
                  <h2 className="text-sm font-semibold text-zinc-300 mb-4 flex items-center gap-2">
                    <Activity className="h-4 w-4 text-cyan-500" />
                    Performance Metrics
                  </h2>

                  <div className="grid grid-cols-2 md:grid-cols-3 gap-3 mb-4">
                    <StatCard
                      label="Win Rate"
                      value={performance.win_rate * 100}
                      format="percent"
                    />
                    <StatCard label="Total P&L" value={performance.total_pnl} format="pnl" />
                    <StatCard label="ROI" value={performance.roi * 100} format="percent" />
                  </div>

                  <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
                    <div className="p-3 rounded-lg bg-white/5 text-center">
                      <p className="text-lg font-bold text-emerald-400">
                        {performance.winning_trades}
                      </p>
                      <p className="text-xs text-zinc-500">Wins</p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5 text-center">
                      <p className="text-lg font-bold text-red-400">{performance.losing_trades}</p>
                      <p className="text-xs text-zinc-500">Losses</p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5 text-center">
                      <p className="text-lg font-bold text-zinc-200">{performance.total_trades}</p>
                      <p className="text-xs text-zinc-500">Total Trades</p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5 text-center">
                      <p className="text-lg font-bold text-zinc-200">
                        {formatPrice(performance.initial_balance)}
                      </p>
                      <p className="text-xs text-zinc-500">Initial Bal</p>
                    </div>
                    <div className="p-3 rounded-lg bg-white/5 text-center">
                      <p
                        className={
                          "text-lg font-bold " +
                          (performance.final_balance >= performance.initial_balance
                            ? "text-emerald-400"
                            : "text-red-400")
                        }
                      >
                        {formatPrice(performance.final_balance)}
                      </p>
                      <p className="text-xs text-zinc-500">Final Bal</p>
                    </div>
                  </div>
                </GlassCard>
              </motion.div>
            )}

            {testEvents && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
              >
                <GlassCard className="p-5">
                  <h2 className="text-sm font-semibold text-zinc-300 mb-4 flex items-center gap-2">
                    <Clock className="h-4 w-4 text-violet-500" />
                    Event Timeline
                  </h2>

                  {testEvents.intents.length > 0 && (
                    <div className="space-y-3 mb-6">
                      <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider">
                        Trade Intents
                      </h3>
                      <div className="space-y-2">
                        {testEvents.intents.map((intent) => (
                          <div
                            key={intent.id}
                            className="flex items-center justify-between p-3 rounded-lg bg-white/5 border border-white/8"
                          >
                            <div className="flex items-center gap-3">
                              {getSignalIcon(intent.side)}
                              <div>
                                <p className="text-sm font-medium text-zinc-200">
                                  {intent.side}{" "}
                                  <span className="text-zinc-500">({intent.strategy_type})</span>
                                </p>
                                <p className="text-xs text-zinc-500">{intent.reason}</p>
                              </div>
                            </div>
                            <div className="text-right">
                              <p
                                className={
                                  "text-xs font-medium " +
                                  (intent.confidence >= 0.7
                                    ? "text-emerald-400"
                                    : intent.confidence >= 0.5
                                      ? "text-amber-400"
                                      : "text-zinc-400")
                                }
                              >
                                {(intent.confidence * 100).toFixed(0)}%
                              </p>
                              <p className="text-xs text-zinc-600">{intent.status}</p>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {testEvents.executions.length > 0 && (
                    <div className="space-y-3">
                      <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider">
                        Executions
                      </h3>
                      <div className="space-y-2">
                        {testEvents.executions.map((exec) => (
                          <div
                            key={exec.id}
                            className="flex items-center justify-between p-3 rounded-lg bg-white/5 border border-white/8"
                          >
                            <div className="flex items-center gap-3">
                              {getSignalIcon(exec.side)}
                              <div>
                                <p className="text-sm font-medium text-zinc-200">
                                  {exec.side} - {exec.filled_size.toFixed(4)} @ $
                                  {exec.avg_fill_price.toFixed(4)}
                                </p>
                                <p className="text-xs text-zinc-500">
                                  {exec.status}
                                  {exec.error_code && " - " + exec.error_code}
                                </p>
                              </div>
                            </div>
                            <p className="text-xs text-zinc-500">
                              {formatTime(new Date(exec.created_at))}
                            </p>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {testEvents.intents.length === 0 && testEvents.executions.length === 0 && (
                    <div className="py-6">
                      <EmptyState
                        title="No events"
                        description="Events from this test will appear here"
                      />
                    </div>
                  )}
                </GlassCard>
              </motion.div>
            )}

            {!testResult && !performance && !testEvents && !error && (
              <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
                <GlassCard className="p-12 flex flex-col items-center justify-center">
                  <div className="py-12">
                    <EmptyState
                      title="Ready to test"
                      description="Select a strategy, enter a market ID, and run a test to see results here"
                    />
                  </div>
                </GlassCard>
              </motion.div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
