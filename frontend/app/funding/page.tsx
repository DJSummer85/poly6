"use client";

import { AnimatePresence, motion } from "framer-motion";
import {
  AlertTriangle,
  ArrowRight,
  Check,
  Copy,
  Loader2,
  RefreshCw,
  Wallet,
  Zap,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { useWallet } from "@/hooks/use-wallet";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";

// Minimal EIP-1193 provider type
interface EIP1193Provider {
  request: (args: { method: string; params?: unknown[] | object }) => Promise<unknown>;
}

function getEthereumProvider(): EIP1193Provider | undefined {
  return (window as { ethereum?: EIP1193Provider }).ethereum;
}

function isRpcError(err: unknown): err is { code?: number; message?: string } {
  return typeof err === "object" && err !== null;
}

// Polygon contract for pUSD wrap
const ONRAMP_ADDRESS = "0x93070a847efEf7F70739046A929D47a521F5B8ee";

export default function FundingPage() {
  const router = useRouter();
  const { token, isAuthenticated } = useAppStore();
  const [loading, setLoading] = useState(true);
  const [backendWallet, setBackendWallet] = useState<string | null>(null);
  const [wrapping, setWrapping] = useState(false);
  const [wrapAmount, setWrapAmount] = useState("100");

  const {
    wallet,
    balances,
    loading: mmLoading,
    connect,
    switchChain,
    refreshBalances,
    hasFunds,
    hasPusd,
    hasGas,
  } = useWallet(backendWallet ?? undefined);

  const loadWalletInfo = useCallback(async () => {
    try {
      const info = await apiFetch<{ wallet_address: string; has_credentials: boolean }>(
        "/funding/wallet-info"
      );
      if (info.wallet_address && info.has_credentials) {
        setBackendWallet(info.wallet_address);
      }
    } catch {
      // Wallet not available yet — user may need to configure credentials
    } finally {
      setLoading(false);
    }
  }, []);

  // Check auth and fetch wallet address from backend
  useEffect(() => {
    const hasToken = typeof window !== "undefined" && (token || localStorage.getItem("token"));
    if (!isAuthenticated && !hasToken) {
      router.push("/login");
      return;
    }

    void loadWalletInfo();
  }, [isAuthenticated, loadWalletInfo, router, token]);

  const handleWrap = useCallback(async () => {
    if (!wallet.address) return;
    setWrapping(true);
    try {
      const amountWei = Math.floor(parseFloat(wrapAmount) * 1e6)
        .toString(16)
        .padStart(64, "0");
      const wrapData = `0xd0e30db0${amountWei}`;

      const ethereum = getEthereumProvider();
      if (!ethereum) {
        toast.error("MetaMask nem található");
        return;
      }
      const txHash = (await ethereum.request({
        method: "eth_sendTransaction",
        params: [{ from: wallet.address, to: ONRAMP_ADDRESS, data: wrapData }],
      })) as string;

      toast.success(`Wrap elküldve! ${txHash.slice(0, 10)}...`);
      setTimeout(() => void refreshBalances(), 5000);
    } catch (err: unknown) {
      if (isRpcError(err) && err.code === 4001) {
        toast.error("Tranzakció elutasítva");
      } else {
        const msg = isRpcError(err) ? err.message : "Wrap sikertelen";
        toast.error(msg);
      }
    } finally {
      setWrapping(false);
    }
  }, [wallet.address, wrapAmount, refreshBalances]);

  const copyAddress = () => {
    const addr = backendWallet ?? wallet.address;
    if (addr) {
      navigator.clipboard.writeText(addr);
      toast.success("Cím másolva!");
    }
  };

  const displayAddress = wallet.address ?? backendWallet;

  if (loading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-[#0b0b0f]">
        <div className="flex flex-col items-center gap-4">
          <div className="h-10 w-10 animate-spin rounded-full border-4 border-indigo-500 border-t-transparent" />
          <p className="text-sm text-zinc-400">Wallet betöltése...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-[#0b0b0f] relative overflow-hidden">
      {/* Ambient glow */}
      <div
        className="ambient-glow ambient-glow-primary absolute"
        style={{ width: 600, height: 600, top: "10%", left: "20%" }}
      />

      <div className="max-w-xl mx-auto px-4 py-8 relative z-10">
        {/* Header */}
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 rounded-xl bg-indigo-500/15 flex items-center justify-center">
            <Wallet size={20} className="text-indigo-400" />
          </div>
          <div>
            <h1 className="text-xl font-bold text-zinc-100">Egyenleg feltöltés</h1>
            <p className="text-xs text-zinc-500">Polymarket trading wallet</p>
          </div>
        </div>

        {/* Wallet Address Card */}
        <AnimatePresence>
          {displayAddress && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              className="glass-card p-4 mb-4"
            >
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs text-zinc-500">Polymarket wallet cím</span>
                <button
                  type="button"
                  onClick={copyAddress}
                  className="flex items-center gap-1 text-xs text-indigo-400 hover:text-indigo-300"
                >
                  <Copy size={12} /> Másolás
                </button>
              </div>
              <code className="text-xs text-zinc-300 break-all font-mono">{displayAddress}</code>
            </motion.div>
          )}
        </AnimatePresence>

        {/* MetaMask connect button */}
        {!wallet.connected ? (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.1 }}
            className="glass-card p-5 mb-4"
          >
            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-xl bg-orange-500/15 flex items-center justify-center">
                <Wallet size={20} className="text-orange-400" />
              </div>
              <div>
                <h2 className="text-lg font-semibold text-zinc-100">MetaMask csatlakozás</h2>
                <p className="text-xs text-zinc-400">
                  Csatlakoztasd a MetaMask-ot az egyenlegek ellenőrzéséhez és a wrapeleshez
                </p>
              </div>
            </div>

            <button
              type="button"
              onClick={connect}
              disabled={mmLoading}
              className="btn-primary w-full flex items-center justify-center gap-2"
            >
              {mmLoading ? <Loader2 size={16} className="animate-spin" /> : <Wallet size={16} />}
              {mmLoading ? "Csatlakozás..." : "MetaMask csatlakozás"}
            </button>
          </motion.div>
        ) : (
          <>
            {/* Wrong chain warning */}
            {wallet.isWrongChain && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="glass-card p-3 mb-4 border border-amber-500/30"
              >
                <div className="flex items-center gap-2">
                  <AlertTriangle size={16} className="text-amber-400" />
                  <span className="text-sm text-amber-300">Nem Polygon hálózat aktív!</span>
                  <button
                    type="button"
                    onClick={switchChain}
                    className="ml-auto text-xs text-indigo-400 hover:text-indigo-300 underline"
                  >
                    Váltás Polygon-ra
                  </button>
                </div>
              </motion.div>
            )}

            {/* Balances */}
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.1 }}
              className="glass-card p-4 mb-4"
            >
              <div className="flex items-center justify-between mb-3">
                <span className="text-xs font-semibold text-zinc-400">EGYENLEG</span>
                <button
                  type="button"
                  onClick={() => void refreshBalances()}
                  className="text-zinc-500 hover:text-indigo-400"
                >
                  <RefreshCw size={12} />
                </button>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <BalanceTile
                  label="USDC.e"
                  value={balances.usdc}
                  color={hasFunds ? "text-green-400" : "text-zinc-500"}
                />
                <BalanceTile
                  label="pUSD"
                  value={balances.pusd}
                  color={hasPusd ? "text-green-400" : "text-zinc-500"}
                />
                <BalanceTile
                  label="MATIC"
                  value={balances.matic}
                  color={hasGas ? "text-green-400" : "text-zinc-500"}
                />
              </div>
            </motion.div>

            {/* Status: no funds yet */}
            {!hasFunds && !hasPusd && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
                className="glass-card p-5 mb-4"
              >
                <div className="flex items-center gap-3 mb-4">
                  <div className="w-10 h-10 rounded-xl bg-indigo-500/15 flex items-center justify-center">
                    <ArrowRight size={20} className="text-indigo-400" />
                  </div>
                  <div>
                    <h2 className="text-lg font-semibold text-zinc-100">Egyenleg feltöltése</h2>
                    <p className="text-xs text-zinc-400">
                      Küldj USDC.e-t Polygon hálózaton erre a címre
                    </p>
                  </div>
                </div>

                <div className="space-y-3">
                  <div className="p-3 rounded-lg bg-zinc-900/50 border border-white/5">
                    <p className="text-xs text-zinc-500 mb-1">Polygon wallet cím:</p>
                    <div className="flex items-center gap-2">
                      <code className="text-xs text-indigo-400 font-mono flex-1 break-all">
                        {displayAddress}
                      </code>
                      <button
                        type="button"
                        onClick={copyAddress}
                        className="p-1.5 rounded-md bg-indigo-500/15 text-indigo-400 hover:bg-indigo-500/25 flex-shrink-0"
                      >
                        <Copy size={14} />
                      </button>
                    </div>
                  </div>

                  <div className="p-3 rounded-lg bg-zinc-900/50 border border-white/5">
                    <p className="text-xs text-zinc-500 mb-1">
                      Fontos: Polygon (MATIC) hálózatot használj!
                    </p>
                    <p className="text-xs text-zinc-600">
                      USDC.e (0x2791...4174) token a Polymarket-en. Binance, Kraken vagy bármely
                      exchange-ről Polygon-ra küldés.
                    </p>
                  </div>

                  <div className="p-3 rounded-lg bg-zinc-900/50 border border-white/5">
                    <p className="text-xs text-zinc-500 mb-1">Gas-hoz kell ~0.1 MATIC is</p>
                    <p className="text-xs text-zinc-600">Ez kell a wrap és trade tranzakciókhoz</p>
                  </div>
                </div>
              </motion.div>
            )}

            {/* Status: has USDC but no pUSD */}
            {hasFunds && !hasPusd && !wallet.isWrongChain && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
                className="glass-card p-5 mb-4"
              >
                <div className="flex items-center gap-3 mb-4">
                  <div className="w-10 h-10 rounded-xl bg-green-500/10 flex items-center justify-center">
                    <Check size={20} className="text-green-400" />
                  </div>
                  <div>
                    <h2 className="text-lg font-semibold text-zinc-100">USDC érkezett!</h2>
                    <p className="text-xs text-zinc-400">Most wrapeld pUSD-ra a tradinghez</p>
                  </div>
                </div>

                {!hasGas ? (
                  <div className="p-3 rounded-lg bg-amber-500/10 border border-amber-500/20 text-center mb-3">
                    <p className="text-sm text-amber-300">Nincs elég MATIC a gas-hoz!</p>
                    <p className="text-xs text-zinc-500 mt-1">
                      Küldj MATIC-ot Polygon-ra a wrap tranzakciókhoz
                    </p>
                  </div>
                ) : (
                  <>
                    <div className="mb-3">
                      <label htmlFor="wrap-amount" className="text-xs text-zinc-500 mb-1 block">
                        Wrap mennyiség (USDC.e → pUSD)
                      </label>
                      <div className="flex gap-2">
                        <input
                          id="wrap-amount"
                          type="number"
                          value={wrapAmount}
                          onChange={(e) => setWrapAmount(e.target.value)}
                          className="input flex-1"
                          min="1"
                          max={balances.usdc}
                          placeholder="0.00"
                        />
                        <button
                          type="button"
                          onClick={() => setWrapAmount(balances.usdc)}
                          className="px-3 rounded-lg text-xs font-medium text-indigo-400 bg-indigo-500/10 border border-indigo-500/20"
                        >
                          MAX
                        </button>
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={handleWrap}
                      disabled={wrapping || parseFloat(wrapAmount) <= 0}
                      className="btn-primary w-full flex items-center justify-center gap-2"
                    >
                      {wrapping ? (
                        <Loader2 size={16} className="animate-spin" />
                      ) : (
                        <Zap size={16} />
                      )}
                      {wrapping ? "Wrap folyamatban..." : `Wrap ${wrapAmount} USDC.e → pUSD`}
                    </button>
                  </>
                )}
              </motion.div>
            )}

            {/* Status: has pUSD — ready to trade */}
            {hasPusd && !wallet.isWrongChain && (
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
                className="glass-card p-5 mb-4 text-center"
              >
                <div className="w-14 h-14 rounded-2xl bg-green-500/10 flex items-center justify-center mx-auto mb-4">
                  <Zap size={24} className="text-green-400" />
                </div>
                <h2 className="text-lg font-semibold text-zinc-100 mb-1">Kész a tradingre!</h2>
                <p className="text-sm text-zinc-400 mb-4">
                  pUSD egyenleg: <strong className="text-green-400">${balances.pusd}</strong>
                </p>

                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={() => router.push("/bots")}
                    className="btn-primary flex-1 flex items-center justify-center gap-2"
                  >
                    Botok <ArrowRight size={14} />
                  </button>
                  <button
                    type="button"
                    onClick={() => router.push("/")}
                    className="btn-primary flex-1 flex items-center justify-center gap-2"
                  >
                    Dashboard <ArrowRight size={14} />
                  </button>
                </div>
              </motion.div>
            )}
          </>
        )}

        {/* Footer info */}
        <motion.p
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.4 }}
          className="text-center text-xs text-zinc-600 mt-4"
        >
          Polygon hálózat • USDC.e → pUSD wrap • Gas: MATIC
        </motion.p>
      </div>
    </div>
  );
}

function BalanceTile({ label, value, color }: { label: string; value: string; color: string }) {
  const icons: Record<string, string> = { "USDC.e": "💵", pUSD: "🔄", MATIC: "⛽" };
  return (
    <div className="text-center p-3 rounded-xl bg-zinc-900/50 border border-white/5">
      <div className="text-lg mb-1">{icons[label] ?? "🪙"}</div>
      <div className={`text-sm font-bold ${color}`}>${parseFloat(value).toFixed(2)}</div>
      <div className="text-xs text-zinc-600">{label}</div>
    </div>
  );
}
