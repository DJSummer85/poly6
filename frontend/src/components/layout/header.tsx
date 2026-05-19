"use client";

import { Bell, Home, BarChart2, ShoppingCart, Settings as SettingsIcon, Bot as BotIcon, LogIn, LogOut, Wallet, User, Zap } from "lucide-react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { toast } from "sonner";
import { useSettings } from "@/hooks";
import { useAppStore } from "@/store";

export function Header() {
  const router = useRouter();
  const pathname = usePathname();
  const { clearAuth, user, isAuthenticated, tradingMode, setTradingMode } = useAppStore();
  const { data: settings } = useSettings();
  const hasCreds = settings?.has_credentials ?? false;

  const handleLogout = () => {
    clearAuth();
    toast.success("Sikeres kijelentkezés");
    router.push("/");
  };

  const handleModeChange = async (mode: "demo" | "live") => {
    setTradingMode(mode);
    try {
      const trading_mode = mode === "live" ? "live" : "paper";
      const { apiFetch } = await import("@/lib/utils");
      await apiFetch("/bots/set-mode", { method: "POST", body: JSON.stringify({ trading_mode }) });
    } catch {}
  };

  const navLinks = [
    { href: "/", label: "Home", icon: Home },
    { href: "/bots", label: "Bots", icon: BotIcon },
    { href: "/markets", label: "Markets", icon: BarChart2 },
    { href: "/orders", label: "Orders", icon: ShoppingCart },
    { href: "/settings", label: "Settings", icon: SettingsIcon },
  ];

  const isActive = (href: string) => {
    if (href === "/") return pathname === "/";
    return pathname === href || pathname?.startsWith(href + "/");
  };

  return (
    <header style={{ borderBottom: '1px solid rgba(255,255,255,0.06)', background: 'rgba(9,9,20,0.97)', backdropFilter: 'blur(20px)', position: 'sticky', top: 0, zIndex: 100 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', maxWidth: '1600px', margin: '0 auto', padding: '0 20px', height: '56px', gap: '16px' }}>

        {/* Logo */}
        <Link href="/" style={{ display: 'flex', alignItems: 'center', gap: '8px', textDecoration: 'none', flexShrink: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', width: '30px', height: '30px', background: '#6366f120', borderRadius: '8px', border: '1px solid #6366f130' }}>
            <Zap size={15} color="#818cf8" fill="#818cf8" />
          </div>
          <span style={{ fontSize: '16px', fontWeight: 800, color: '#e2e8f0' }}>
            Poly<span style={{ color: '#6366f1' }}>Trade</span>
          </span>
        </Link>

        {/* Nav links */}
        <nav style={{ display: 'flex', alignItems: 'center', gap: '2px' }}>
          {navLinks.map(({ href, label, icon: Icon }) => {
            const active = isActive(href);
            return (
              <Link
                key={href}
                href={href}
                style={{
                  display: 'flex', alignItems: 'center', gap: '6px',
                  padding: '6px 12px', borderRadius: '8px',
                  fontSize: '13px', fontWeight: active ? 700 : 500,
                  color: active ? '#e2e8f0' : '#6b7280',
                  background: active ? '#6366f120' : 'transparent',
                  border: active ? '1px solid #6366f130' : '1px solid transparent',
                  textDecoration: 'none', transition: 'all 0.15s ease'
                }}
              >
                <Icon size={14} />
                {label}
              </Link>
            );
          })}
        </nav>

        {/* Right side: DEMO/LIVE + user + wallet + bell + logout */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 }}>

          {/* DEMO / LIVE toggle */}
          <div style={{ display: 'flex', background: '#0d0d1a', padding: '3px', borderRadius: '8px', border: '1px solid #1e1e30', gap: '2px' }}>
            <button
              onClick={() => handleModeChange("demo")}
              style={{
                padding: '5px 12px', fontSize: '11px', fontWeight: 800, borderRadius: '6px',
                background: tradingMode === 'demo' ? '#6366f1' : 'transparent',
                border: 'none', color: tradingMode === 'demo' ? '#fff' : '#6b7280',
                cursor: 'pointer', transition: 'all 0.15s ease'
              }}
            >DEMO</button>
            <button
              onClick={() => handleModeChange("live")}
              style={{
                padding: '5px 12px', fontSize: '11px', fontWeight: 800, borderRadius: '6px',
                background: tradingMode === 'live' ? '#22c55e' : 'transparent',
                border: 'none', color: tradingMode === 'live' ? '#fff' : '#6b7280',
                cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px',
                transition: 'all 0.15s ease'
              }}
            >
              {tradingMode === 'live' && <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: '#fff', display: 'inline-block' }} />}
              LIVE
            </button>
          </div>

          {/* User chip */}
          {isAuthenticated && user ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px', background: '#22c55e10', border: '1px solid #22c55e20', borderRadius: '8px', padding: '5px 10px' }}>
              <div style={{ width: '6px', height: '6px', borderRadius: '50%', background: '#22c55e' }} />
              <User size={11} color="#4ade80" />
              <span style={{ fontSize: '11px', fontWeight: 700, color: '#4ade80' }}>@{user.username}</span>
            </div>
          ) : (
            <Link href="/login" style={{ display: 'flex', alignItems: 'center', gap: '6px', background: '#ffffff08', border: '1px solid #ffffff10', borderRadius: '8px', padding: '5px 10px', textDecoration: 'none' }}>
              <LogIn size={12} color="#6b7280" />
              <span style={{ fontSize: '11px', fontWeight: 600, color: '#6b7280' }}>Bejelentkezés</span>
            </Link>
          )}

          {/* Wallet chip */}
          {isAuthenticated && (
            hasCreds ? (
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', background: '#f59e0b10', border: '1px solid #f59e0b25', borderRadius: '8px', padding: '5px 10px' }}>
                <Wallet size={11} color="#fbbf24" />
                <span style={{ fontSize: '11px', fontWeight: 700, color: '#fbbf24' }}>Connect</span>
              </div>
            ) : (
              <Link href="/settings" style={{ display: 'flex', alignItems: 'center', gap: '6px', background: '#f59e0b10', border: '1px solid #f59e0b25', borderRadius: '8px', padding: '5px 10px', textDecoration: 'none' }}>
                <Wallet size={11} color="#fbbf24" />
                <span style={{ fontSize: '11px', fontWeight: 700, color: '#fbbf24' }}>Connect</span>
              </Link>
            )
          )}

          {/* Bell */}
          <button style={{ width: '34px', height: '34px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#ffffff05', border: '1px solid #ffffff08', borderRadius: '8px', cursor: 'pointer' }}>
            <Bell size={14} color="#6b7280" />
          </button>

          {/* Logout */}
          {isAuthenticated ? (
            <button
              onClick={handleLogout}
              style={{ width: '34px', height: '34px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#ffffff05', border: '1px solid #ffffff08', borderRadius: '8px', cursor: 'pointer' }}
            >
              <LogOut size={14} color="#6b7280" />
            </button>
          ) : (
            <Link href="/login" style={{ width: '34px', height: '34px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#ffffff05', border: '1px solid #ffffff08', borderRadius: '8px' }}>
              <LogIn size={14} color="#6b7280" />
            </Link>
          )}
        </div>
      </div>
    </header>
  );
}
