"use client";

import { AnimatePresence, motion } from "framer-motion";
import {
  AlertTriangle,
  Check,
  Edit3,
  Eye,
  EyeOff,
  Key,
  Loader2,
  Settings,
  Shield,
  Trash2,
  Wallet,
  X,
  Zap,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";

type ApiKeyType = {
  id: string;
  name: string;
  description: string;
  fields: { key: string; label: string; placeholder: string; required: boolean }[];
  status: "empty" | "valid" | "invalid" | "validating";
};

const API_KEYS_CONFIG: ApiKeyType[] = [
  {
    id: "polymarket",
    name: "Polymarket API",
    description: "Kereskedési bot API kulcsok a Polymarket platformhoz",
    fields: [
      { key: "api_key", label: "API Key", placeholder: "pm_api_xxxxx (üresen hagyható – auto-deriválás)", required: false },
      { key: "api_secret", label: "API Secret", placeholder: "pm_secret_xxxxx (üresen hagyható – auto-deriválás)", required: false },
      { key: "passphrase", label: "Passphrase", placeholder: "your_passphrase (üresen hagyható – auto-deriválás)", required: false },
      { key: "private_key", label: "Private Key", placeholder: "0x...", required: true },
      { key: "signature_type", label: "Signature Type (0=Metamask, 1=Email/Magic, 3=Deposit Wallet/POLY_1271)", placeholder: "3", required: false },
      { key: "funder", label: "Profile Address (Optional)", placeholder: "0x...", required: false },
    ],
    status: "empty",
  },
  {
    id: "binance",
    name: "Binance API",
    description: "BTC árfolyam lekérdezés és trading szignalálás",
    fields: [
      { key: "api_key", label: "API Key", placeholder: "bn_api_xxxxx", required: true },
      { key: "api_secret", label: "API Secret", placeholder: "bn_secret_xxxxx", required: true },
    ],
    status: "empty",
  },
  {
    id: "telegram",
    name: "Telegram Értesítések",
    description: "Valós idejű értesítések küldése a bot tevékenységéről",
    fields: [
      { key: "bot_token", label: "Bot Token", placeholder: "123456:ABC-DEF...", required: true },
      { key: "chat_id", label: "Chat ID", placeholder: "123456789", required: true },
    ],
    status: "empty",
  },
];

type StoredKey = {
  key_name: string;
  key_value: string;
  is_valid: boolean;
};

export default function SettingsPage() {
  const router = useRouter();
  const { isAuthenticated } = useAppStore();
  const [apiKeys, setApiKeys] = useState<ApiKeyType[]>(API_KEYS_CONFIG);
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [inputValues, setInputValues] = useState<Record<string, Record<string, string>>>({
    polymarket: { api_key: "", api_secret: "", passphrase: "", private_key: "", signature_type: "0", funder: "" },
    binance: { api_key: "", api_secret: "" },
    telegram: { bot_token: "", chat_id: "" },
  });
  const [showValues, setShowValues] = useState<Record<string, Record<string, boolean>>>({
    polymarket: { api_key: false, api_secret: false, passphrase: false, private_key: false },
    binance: { api_key: false, api_secret: false },
    telegram: { bot_token: false, chat_id: false },
  });
  const [loading, setLoading] = useState(false);

  const loadStoredKeys = useCallback(async () => {
    try {
      const keys = await apiFetch<StoredKey[]>("/settings/keys", { method: "GET" });

      // Update status for each API key config
      setApiKeys((prev) =>
        prev.map((config) => {
          const relatedKeys = keys.filter((k) => k.key_name.startsWith(`${config.id}_`));
          if (relatedKeys.length === 0) return { ...config, status: "empty" };
          const allValid = relatedKeys.every((k) => k.is_valid);
          return { ...config, status: allValid ? "valid" : "invalid" };
        })
      );

      // Populate input values from stored keys
      setInputValues((prev) => {
        const nextValues = structuredClone(prev);
        keys.forEach((stored) => {
          const [provider, ...fieldParts] = stored.key_name.split("_");
          const field = fieldParts.join("_");
          if (nextValues[provider] && field) {
            nextValues[provider][field] = stored.key_value;
          }
        });
        return nextValues;
      });
    } catch (err) {
      console.error("Failed to load stored keys:", err);
    }
  }, []);

  useEffect(() => {
    // Check both store state and localStorage for auth
    const hasToken = typeof window !== "undefined" && localStorage.getItem("token");
    if (!isAuthenticated && !hasToken) {
      router.push("/login");
      return;
    }
    void loadStoredKeys();
  }, [isAuthenticated, loadStoredKeys, router]);

  const validateKey = async (provider: string, field: string, value: string) => {
    if (!value) return false;

    setApiKeys((prev) =>
      prev.map((config) => (config.id === provider ? { ...config, status: "validating" } : config))
    );

    try {
      const result = await apiFetch<{ valid: boolean; message?: string }>("/settings/validate", {
        method: "POST",
        body: JSON.stringify({ key_name: `${provider}_${field}`, key_value: value }),
      });

      if (!result.valid) {
        toast.error(result.message || `${field} validálása sikertelen`);
        setApiKeys((prev) =>
          prev.map((config) => (config.id === provider ? { ...config, status: "invalid" } : config))
        );
        return false;
      }

      return true;
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Validálási hiba");
      setApiKeys((prev) =>
        prev.map((config) => (config.id === provider ? { ...config, status: "invalid" } : config))
      );
      return false;
    }
  };

  const saveKey = async (provider: string) => {
    setLoading(true);
    const values = inputValues[provider];
    const config = apiKeys.find((c) => c.id === provider);

    if (!config) return;

    // Validate required fields first
    for (const field of config.fields) {
      if (field.required && !values[field.key]) {
        toast.error(`${field.label} megadása kötelező`);
        setLoading(false);
        return;
      }
    }

    // For Polymarket: use PUT /settings which auto-derives the API key from private key
    if (provider === "polymarket") {
      try {
        const password = prompt("Add meg a jelszavadat a kulcsok titkosításához:") || "default";
        const body: Record<string, unknown> = {
          polymarket_private_key: values["private_key"],
          password,
          signature_type: values["signature_type"] ? parseInt(values["signature_type"]) : 0,
          funder: values["funder"] || null,
        };
        const result = await apiFetch<{ success?: boolean; error?: string; wallet_address?: string }>("/settings", {
          method: "PUT",
          body: JSON.stringify(body),
        });
        if (result && (result as any).error) {
          toast.error(`Hiba: ${(result as any).error}`);
          setApiKeys((prev) => prev.map((c) => (c.id === provider ? { ...c, status: "invalid" } : c)));
        } else {
          toast.success("Polymarket kulcsok sikeresen deriválva és mentve!");
          setApiKeys((prev) => prev.map((c) => (c.id === provider ? { ...c, status: "valid" } : c)));
          setEditingKey(null);
          await loadStoredKeys();
        }
      } catch (err) {
        toast.error(err instanceof Error ? err.message : "Mentés sikertelen");
        setApiKeys((prev) => prev.map((c) => (c.id === provider ? { ...c, status: "invalid" } : c)));
      } finally {
        setLoading(false);
      }
      return;
    }

    // For other providers: save fields individually
    try {
      for (const field of config.fields) {
        if (values[field.key]) {
          await apiFetch("/settings/store", {
            method: "POST",
            body: JSON.stringify({
              key_name: `${provider}_${field.key}`,
              key_value: values[field.key],
            }),
          });
        }
      }

      toast.success(`${config.name} kulcsok sikeresen mentve!`);
      setApiKeys((prev) => prev.map((c) => (c.id === provider ? { ...c, status: "valid" } : c)));
      setEditingKey(null);
      await loadStoredKeys();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Mentés sikertelen");
      setApiKeys((prev) => prev.map((c) => (c.id === provider ? { ...c, status: "invalid" } : c)));
    } finally {
      setLoading(false);
    }
  };

  const deleteKey = async (provider: string) => {
    try {
      await apiFetch(`/settings/keys/${provider}`, { method: "DELETE" });
      toast.success(`${provider} kulcsok törölve`);

      // Clear input values
      setInputValues((prev) => {
        const clearedFields: Record<string, string> = {};
        for (const field of apiKeys.find((c) => c.id === provider)?.fields ?? []) {
          clearedFields[field.key] = "";
        }

        return {
          ...prev,
          [provider]: clearedFields,
        };
      });

      setApiKeys((prev) => prev.map((c) => (c.id === provider ? { ...c, status: "empty" } : c)));
      await loadStoredKeys();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Törlés sikertelen");
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "valid":
        return <Check size={16} style={{ color: "#22c55e" }} />;
      case "invalid":
        return <X size={16} style={{ color: "#ef4444" }} />;
      case "validating":
        return <Loader2 size={16} className="animate-spin" style={{ color: "#6366f1" }} />;
      default:
        return <AlertTriangle size={16} style={{ color: "#71717a" }} />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "valid":
        return "#22c55e";
      case "invalid":
        return "#ef4444";
      case "validating":
        return "#6366f1";
      default:
        return "#71717a";
    }
  };

  return (
    <div
      style={{
        minHeight: "100vh",
        background: "#0b0b0f",
        padding: "2rem",
        position: "relative",
      }}
    >
      {/* Ambient glow */}
      <div
        className="ambient-glow ambient-glow-primary"
        style={{ width: 600, height: 600, top: "10%", left: "20%" }}
      />
      <div
        className="ambient-glow ambient-glow-blue"
        style={{ width: 400, height: 400, bottom: "20%", right: "10%" }}
      />

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        style={{ maxWidth: 800, margin: "0 auto" }}
      >
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: "1rem", marginBottom: "2rem" }}>
          <div
            style={{
              width: 48,
              height: 48,
              borderRadius: 12,
              background: "rgba(99, 102, 241, 0.15)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Settings size={24} style={{ color: "#6366f1" }} />
          </div>
          <div>
            <h1 style={{ fontWeight: 700, fontSize: 24, color: "#fafafa" }}>Beállítások</h1>
            <span style={{ fontSize: 14, color: "#71717a" }}>API kulcsok és konfiguráció</span>
          </div>
        </div>

        {/* Security notice */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          style={{
            padding: "1rem",
            borderRadius: 12,
            background: "rgba(99, 102, 241, 0.1)",
            border: "1px solid rgba(99, 102, 241, 0.2)",
            marginBottom: "2rem",
            display: "flex",
            alignItems: "center",
            gap: "1rem",
          }}
        >
          <Shield size={20} style={{ color: "#6366f1" }} />
          <div>
            <span style={{ fontSize: 14, fontWeight: 600, color: "#fafafa" }}>
              Biztonsági figyelmeztetés
            </span>
            <span style={{ fontSize: 12, color: "#71717a", display: "block", marginTop: 4 }}>
              API kulcsok titkosítva vannak tárolva. Soha ne ossza meg másokkal!
            </span>
          </div>
        </motion.div>

        {/* API Keys sections */}
        <AnimatePresence>
          {apiKeys.map((config) => (
            <motion.div
              key={config.id}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              className="glass-card"
              style={{ padding: "1.5rem", marginBottom: "1.5rem" }}
            >
              {/* Section header */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  marginBottom: "1rem",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                  <Key size={20} style={{ color: "#6366f1" }} />
                  <div>
                    <h3 style={{ fontWeight: 600, fontSize: 16, color: "#fafafa" }}>
                      {config.name}
                    </h3>
                    <span style={{ fontSize: 12, color: "#71717a" }}>{config.description}</span>
                  </div>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                  {getStatusIcon(config.status)}
                  <span style={{ fontSize: 12, color: getStatusColor(config.status) }}>
                    {config.status === "empty"
                      ? "Nincs beállítva"
                      : config.status === "valid"
                        ? "Aktív"
                        : config.status === "invalid"
                          ? "Hibás"
                          : "Validálás..."}
                  </span>
                </div>
              </div>

              {/* Input fields */}
              <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
                {config.fields.map((field) => (
                  <div key={field.key}>
                    <label
                      htmlFor={`${config.id}-${field.key}`}
                      style={{
                        fontSize: 14,
                        color: "#a1a1aa",
                        marginBottom: "0.5rem",
                        display: "block",
                      }}
                    >
                      {field.label}
                      {field.required && <span style={{ color: "#ef4444" }}> *</span>}
                    </label>
                    <div style={{ position: "relative" }}>
                      <input
                        id={`${config.id}-${field.key}`}
                        type={showValues[config.id]?.[field.key] ? "text" : "password"}
                        value={inputValues[config.id]?.[field.key] || ""}
                        onChange={(e) =>
                          setInputValues((prev) => ({
                            ...prev,
                            [config.id]: { ...prev[config.id], [field.key]: e.target.value },
                          }))
                        }
                        disabled={editingKey !== config.id && config.status === "valid"}
                        className="input"
                        style={{ paddingRight: 40 }}
                        placeholder={field.placeholder}
                      />
                      <button
                        type="button"
                        onClick={() =>
                          setShowValues((prev) => ({
                            ...prev,
                            [config.id]: {
                              ...prev[config.id],
                              [field.key]: !prev[config.id]?.[field.key],
                            },
                          }))
                        }
                        style={{
                          position: "absolute",
                          right: 12,
                          top: "50%",
                          transform: "translateY(-50%)",
                          background: "none",
                          border: "none",
                          cursor: "pointer",
                          color: "#71717a",
                          padding: 4,
                        }}
                      >
                        {showValues[config.id]?.[field.key] ? (
                          <EyeOff size={16} />
                        ) : (
                          <Eye size={16} />
                        )}
                      </button>
                    </div>
                  </div>
                ))}
              </div>

              {/* Action buttons */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "flex-end",
                  gap: "0.5rem",
                  marginTop: "1.5rem",
                }}
              >
                {config.id === "polymarket" && config.status === "valid" && (
                  <DepositWalletInput />
                )}

                {config.id === "telegram" && config.status === "valid" && (
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        const res = await apiFetch<any>("/settings/telegram/test", { method: "POST" });
                        if (res.success) toast.success(res.message);
                        else toast.error(res.error);
                      } catch (e) { toast.error("Hiba a teszt során"); }
                    }}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "0.5rem",
                      padding: "0.5rem 1rem",
                      borderRadius: 8,
                      fontSize: 14,
                      color: "#a3e635",
                      background: "rgba(163, 230, 53, 0.1)",
                      border: "1px solid rgba(163, 230, 53, 0.2)",
                      cursor: "pointer",
                      marginRight: "auto"
                    }}
                  >
                    <Zap size={16} />
                    Teszt üzenet
                  </button>
                )}

                {config.status !== "empty" && (
                  <button
                    type="button"
                    onClick={() => deleteKey(config.id)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "0.5rem",
                      padding: "0.5rem 1rem",
                      borderRadius: 8,
                      fontSize: 14,
                      color: "#ef4444",
                      background: "rgba(239, 68, 68, 0.1)",
                      border: "1px solid rgba(239, 68, 68, 0.2)",
                      cursor: "pointer",
                    }}
                  >
                    <Trash2 size={16} />
                    Törlés
                  </button>
                )}

                {editingKey !== config.id && config.status === "valid" ? (
                  <button
                    type="button"
                    onClick={() => setEditingKey(config.id)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "0.5rem",
                      padding: "0.5rem 1rem",
                      borderRadius: 8,
                      fontSize: 14,
                      color: "#6366f1",
                      background: "rgba(99, 102, 241, 0.1)",
                      border: "1px solid rgba(99, 102, 241, 0.2)",
                      cursor: "pointer",
                    }}
                  >
                    <Edit3 size={16} />
                    Szerkesztés
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => saveKey(config.id)}
                    disabled={loading}
                    className="btn-primary"
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "0.5rem",
                      padding: "0.5rem 1rem",
                    }}
                  >
                    {loading ? <Loader2 size={16} className="animate-spin" /> : <Check size={16} />}
                    Mentés és validálás
                  </button>
                )}
              </div>
            </motion.div>
          ))}
        </AnimatePresence>

        {/* Instructions */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { delay: 0.3 } }}
          style={{
            padding: "1.5rem",
            borderRadius: 12,
            background: "rgba(20, 20, 28, 0.6)",
            border: "1px solid rgba(255, 255, 255, 0.08)",
          }}
        >
          <h4 style={{ fontWeight: 600, fontSize: 14, color: "#fafafa", marginBottom: "0.75rem" }}>
            API kulcsok beszerzése
          </h4>
          <ul style={{ fontSize: 12, color: "#71717a", lineHeight: 1.8, paddingLeft: "1rem" }}>
            <li>
              <strong style={{ color: "#a1a1aa" }}>Polymarket:</strong> Lépjen a{" "}
              <a
                href="https://polymarket.com/settings/api"
                target="_blank"
                rel="noopener noreferrer"
                style={{ color: "#6366f1" }}
              >
                Polymarket API Settings
              </a>{" "}
              oldalra
            </li>
            <li>
              <strong style={{ color: "#a1a1aa" }}>Binance:</strong> Lépjen a{" "}
              <a
                href="https://www.binance.com/en/my/settings/api-management"
                target="_blank"
                rel="noopener noreferrer"
                style={{ color: "#6366f1" }}
              >
                Binance API Management
              </a>{" "}
              oldalra
            </li>
          </ul>
        </motion.div>
      </motion.div>
    </div>
  );
}

// ─── Deposit Wallet Address Input ──────────────────────────────────────

function DepositWalletInput() {
  const [saving, setSaving] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [address, setAddress] = useState("");
  const [savedAddress, setSavedAddress] = useState<string | null>(null);

  // Load saved address on mount
  useEffect(() => {
    const loadSaved = async () => {
      try {
        const info = await apiFetch<{
          deposit_wallet_address: string | null;
          message: string;
        }>("/funding/deposit-wallet-info", { method: "GET" });
        if (info.deposit_wallet_address) {
          setSavedAddress(info.deposit_wallet_address);
          setAddress(info.deposit_wallet_address);
        }
      } catch {
        // Not saved yet, ignore
      }
    };
    loadSaved();
  }, []);

  const handleSave = async () => {
    if (saving || !address.trim()) return;
    setSaving(true);

    try {
      const result = await apiFetch<{ success: boolean; wallet_address?: string; message: string }>(
        "/funding/save-deposit-wallet",
        {
          method: "POST",
          body: JSON.stringify({ wallet_address: address.trim() }),
        }
      );

      if (result.success) {
        setSavedAddress(result.wallet_address ?? address.trim());
        toast.success(result.message);
      } else {
        toast.error(result.message || "Failed to save deposit wallet address");
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save");
    } finally {
      setSaving(false);
    }
  };

  const handleFetch = async () => {
    if (fetching) return;
    setFetching(true);

    try {
      const result = await apiFetch<{ success: boolean; wallet_address?: string; message: string }>(
        "/funding/fetch-deposit-wallet",
        { method: "POST" }
      );

      if (result.success && result.wallet_address) {
        setAddress(result.wallet_address);
        setSavedAddress(result.wallet_address);
        toast.success(result.message);
      } else {
        toast.error(result.message || "No deposit wallet found on Polymarket");
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to fetch");
    } finally {
      setFetching(false);
    }
  };

  return (
    <div style={{ marginRight: "auto", display: "flex", flexDirection: "column", gap: "0.5rem", minWidth: 420 }}>
      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
        <Wallet size={16} style={{ color: "#a1a1aa" }} />
        <span style={{ fontSize: 12, color: "#a1a1aa" }}>
          Deposit Wallet Address (required for signature type 3)
        </span>
      </div>
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center", flexWrap: "wrap" }}>
        <input
          type="text"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          placeholder="0x... paste address manually or use auto-fetch"
          className="input"
          style={{
            flex: 1,
            minWidth: 200,
            fontSize: 13,
            fontFamily: "monospace",
            color: savedAddress && savedAddress === address.trim() ? "#22c55e" : undefined,
          }}
        />
        <button
          type="button"
          onClick={handleSave}
          disabled={saving || !address.trim()}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "0.5rem",
            padding: "0.5rem 1rem",
            borderRadius: 8,
            fontSize: 14,
            color: savedAddress && savedAddress === address.trim() ? "#22c55e" : "#6366f1",
            background: savedAddress && savedAddress === address.trim()
              ? "rgba(34, 197, 94, 0.1)"
              : "rgba(99, 102, 241, 0.1)",
            border: savedAddress && savedAddress === address.trim()
              ? "1px solid rgba(34, 197, 94, 0.2)"
              : "1px solid rgba(99, 102, 241, 0.2)",
            cursor: (saving || !address.trim()) ? "not-allowed" : "pointer",
            whiteSpace: "nowrap",
            opacity: saving ? 0.6 : 1,
          }}
        >
          {saving ? <Loader2 size={16} className="animate-spin" /> : <Check size={16} />}
          {saving ? "Saving..." : savedAddress && savedAddress === address.trim() ? "Saved ✓" : "Save"}
        </button>
        <button
          type="button"
          onClick={handleFetch}
          disabled={fetching}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "0.5rem",
            padding: "0.5rem 1rem",
            borderRadius: 8,
            fontSize: 14,
            color: "#a3e635",
            background: "rgba(163, 230, 53, 0.1)",
            border: "1px solid rgba(163, 230, 53, 0.2)",
            cursor: fetching ? "not-allowed" : "pointer",
            whiteSpace: "nowrap",
            opacity: fetching ? 0.6 : 1,
          }}
        >
          {fetching ? <Loader2 size={16} className="animate-spin" /> : <Zap size={16} />}
          {fetching ? "Fetching..." : "Auto-fetch"}
        </button>
      </div>
      <span style={{ fontSize: 11, color: "#52525b" }}>
        Auto-fetch from the Polymarket Relayer API, or manually paste from{" "}
        <a
          href="https://polymarket.com/settings/api"
          target="_blank"
          rel="noopener noreferrer"
          style={{ color: "#6366f1" }}
        >
          Polymarket.com Settings → API Keys
        </a>
      </span>
    </div>
  );
}
