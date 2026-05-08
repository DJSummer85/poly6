"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

// Polygon mainnet config
const POLYGON_CHAIN_ID = "0x89"; // 137 in hex

const POLYGON_PARAMS = {
  chainId: POLYGON_CHAIN_ID,
  chainName: "Polygon Mainnet",
  nativeCurrency: { name: "MATIC", symbol: "MATIC", decimals: 18 },
  rpcUrls: ["https://polygon-rpc.com", "https://rpc-mainnet.matic.network"],
  blockExplorerUrls: ["https://polygonscan.com"],
};

// ERC-20 token addresses on Polygon
const TOKENS = {
  USDC_E: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174",
  PUSD: "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB",
};

// ERC-20 balanceOf(address) function selector
const BALANCE_OF_SELECTOR = "0x70a08231";

// Minimal EIP-1193 provider type
interface EIP1193Provider {
  isMetaMask?: boolean;
  request: (args: { method: string; params?: unknown[] | object }) => Promise<unknown>;
  on: (event: string, callback: (payload: unknown) => void) => void;
  removeListener: (event: string, callback: (payload: unknown) => void) => void;
}

function getEthereumProvider(): EIP1193Provider | undefined {
  return (window as { ethereum?: EIP1193Provider }).ethereum;
}

function isRpcError(err: unknown): err is { code?: number; message?: string } {
  return typeof err === "object" && err !== null;
}

interface WalletState {
  connected: boolean;
  address: string | null;
  chainId: string | null;
  isMetaMask: boolean;
  isWrongChain: boolean;
}

interface Balances {
  matic: string;
  usdc: string;
  pusd: string;
}

function encodeAddressParam(addr: string): string {
  return addr.toLowerCase().replace("0x", "").padStart(64, "0");
}

async function getChainId(provider: EIP1193Provider): Promise<string> {
  return (await provider.request({ method: "eth_chainId" })) as string;
}

async function switchToPolygon(provider: EIP1193Provider): Promise<boolean> {
  try {
    await provider.request({
      method: "wallet_switchEthereumChain",
      params: [{ chainId: POLYGON_CHAIN_ID }],
    });
    return true;
  } catch (switchError: unknown) {
    // Chain not added yet
    if (isRpcError(switchError) && switchError.code === 4902) {
      try {
        await provider.request({
          method: "wallet_addEthereumChain",
          params: [POLYGON_PARAMS],
        });
        return true;
      } catch (addError: unknown) {
        const msg = isRpcError(addError) ? addError.message : "ismeretlen hiba";
        toast.error(`Polygon hozzáadása sikertelen: ${msg}`);
        return false;
      }
    }
    const msg = isRpcError(switchError) ? switchError.message : "ismeretlen hiba";
    toast.error(`Chain váltás sikertelen: ${msg}`);
    return false;
  }
}

async function fetchBalance(provider: EIP1193Provider, address: string): Promise<Balances> {
  // MATIC (native balance) — use BigInt to avoid precision loss
  const maticHex = (await provider.request({
    method: "eth_getBalance",
    params: [address, "latest"],
  })) as string;
  const matic = (Number(BigInt(maticHex)) / 1e18).toFixed(4);

  // ERC-20 balances via eth_call
  async function getTokenBalance(tokenAddr: string): Promise<number> {
    const data = `${BALANCE_OF_SELECTOR}${encodeAddressParam(address)}`;
    const result = (await provider.request({
      method: "eth_call",
      params: [{ to: tokenAddr, data }, "latest"],
    })) as string;
    // eth_call can return "0x" for zero balance
    if (!result || result === "0x") return 0;
    return Number(BigInt(result)) / 1e6; // USDC/pUSD have 6 decimals
  }

  let usdc = "0.00";
  let pusd = "0.00";
  try {
    usdc = (await getTokenBalance(TOKENS.USDC_E)).toFixed(2);
  } catch (e: unknown) {
    const msg = isRpcError(e) ? e.message : "ismeretlen hiba";
    toast.warning(`USDC egyenleg lekérése sikertelen: ${msg}`);
  }
  try {
    pusd = (await getTokenBalance(TOKENS.PUSD)).toFixed(2);
  } catch (e: unknown) {
    const msg = isRpcError(e) ? e.message : "ismeretlen hiba";
    toast.warning(`pUSD egyenleg lekérése sikertelen: ${msg}`);
  }

  return { matic, usdc, pusd };
}

export function useWallet(expectedAddress?: string) {
  const [wallet, setWallet] = useState<WalletState>({
    connected: false,
    address: null,
    chainId: null,
    isMetaMask: false,
    isWrongChain: false,
  });
  const [balances, setBalances] = useState<Balances>({ matic: "0", usdc: "0", pusd: "0" });
  const [loading, setLoading] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // Use refs to break the dependency cycle that causes infinite loops
  const walletRef = useRef(wallet);
  walletRef.current = wallet;

  const refreshBalances = useCallback(async () => {
    const w = walletRef.current;
    if (!w.connected || !w.address) return;
    const ethereum = getEthereumProvider();
    if (!ethereum) return;
    try {
      const newBalances = await fetchBalance(ethereum, w.address);
      setBalances(newBalances);
    } catch {
      // Ignore transient errors
    }
  }, []);

  // Poll balances every 10s
  const startPolling = useCallback(() => {
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(() => void refreshBalances(), 10_000);
  }, [refreshBalances]);

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  // Try to reconnect on mount (if already approved)
  const checkConnection = useCallback(async () => {
    const ethereum = getEthereumProvider();
    if (!ethereum) return;

    try {
      const accounts = (await ethereum.request({ method: "eth_accounts" })) as string[];
      if (accounts.length > 0) {
        const chainId: string = await getChainId(ethereum);
        setWallet({
          connected: true,
          address: accounts[0],
          chainId,
          isMetaMask: ethereum.isMetaMask ?? true,
          isWrongChain: chainId !== POLYGON_CHAIN_ID,
        });
        void refreshBalances();
        startPolling();
      }
    } catch {
      // Not connected
    }
  }, [refreshBalances, startPolling]);

  // Listen for MetaMask events — deps are now stable (no wallet/state deps)
  useEffect(() => {
    if (typeof window === "undefined") return;
    const ethereum = getEthereumProvider();
    if (!ethereum) return;

    const handleAccountsChanged = (payload: unknown) => {
      const accounts = payload as string[];
      if (accounts.length === 0) {
        setWallet({
          connected: false,
          address: null,
          chainId: null,
          isMetaMask: true,
          isWrongChain: false,
        });
        stopPolling();
        toast.info("MetaMask: fiók leválasztva");
      } else {
        setWallet((prev) => ({ ...prev, address: accounts[0] }));
        void refreshBalances();
      }
    };

    const handleChainChanged = () => {
      void checkConnection();
    };

    ethereum.on("accountsChanged", handleAccountsChanged);
    ethereum.on("chainChanged", handleChainChanged);

    return () => {
      ethereum.removeListener("accountsChanged", handleAccountsChanged);
      ethereum.removeListener("chainChanged", handleChainChanged);
    };
  }, [checkConnection, refreshBalances, stopPolling]);

  // Check connection on mount
  useEffect(() => {
    void checkConnection();
    return () => stopPolling();
  }, [checkConnection, stopPolling]);

  const connect = useCallback(async () => {
    const ethereum = getEthereumProvider();
    if (!ethereum) {
      toast.error("MetaMask nincs telepítve! Telepítsd: https://metamask.io");
      return;
    }

    setLoading(true);
    try {
      const accounts = (await ethereum.request({ method: "eth_requestAccounts" })) as string[];
      if (accounts.length === 0) {
        toast.error("Nincs fiók kiválasztva");
        return;
      }

      // Switch to Polygon
      const switched = await switchToPolygon(ethereum);
      const chainId: string = await getChainId(ethereum);

      // Check if address matches expected
      const connectedAddr = accounts[0];
      const wrongChain = !switched || chainId !== POLYGON_CHAIN_ID;

      if (expectedAddress && connectedAddr.toLowerCase() !== expectedAddress.toLowerCase()) {
        toast.warning("A MetaMask wallet nem egyezik a Polymarket wallet-címmel!");
        toast.info(`Szükséges cím: ${expectedAddress}`);
      }

      setWallet({
        connected: true,
        address: connectedAddr,
        chainId,
        isMetaMask: ethereum.isMetaMask ?? true,
        isWrongChain: wrongChain,
      });

      toast.success("MetaMask csatlakoztatva!");
      void refreshBalances();
      startPolling();
    } catch (err: unknown) {
      if (isRpcError(err) && err.code === 4001) {
        toast.error("MetaMask: felhasználó elutasította");
      } else {
        const msg = isRpcError(err) ? err.message : "Csatlakozás sikertelen";
        toast.error(msg);
      }
    } finally {
      setLoading(false);
    }
  }, [expectedAddress, refreshBalances, startPolling]);

  const disconnect = useCallback(() => {
    setWallet({
      connected: false,
      address: null,
      chainId: null,
      isMetaMask: wallet.isMetaMask,
      isWrongChain: false,
    });
    stopPolling();
    setBalances({ matic: "0", usdc: "0", pusd: "0" });
    toast.info("MetaMask leválasztva");
  }, [wallet.isMetaMask, stopPolling]);

  const switchChain = useCallback(async () => {
    const ethereum = getEthereumProvider();
    if (!ethereum) return;
    const ok = await switchToPolygon(ethereum);
    if (ok) {
      const chainId = await getChainId(ethereum);
      setWallet((prev) => ({ ...prev, isWrongChain: false, chainId }));
      toast.success("Polygon hálózat aktív");
    }
  }, []);

  return {
    wallet,
    balances,
    loading,
    connect,
    disconnect,
    switchChain,
    refreshBalances,
    hasFunds: parseFloat(balances.usdc) > 0,
    hasPusd: parseFloat(balances.pusd) > 0,
    hasGas: parseFloat(balances.matic) > 0.01,
  };
}
