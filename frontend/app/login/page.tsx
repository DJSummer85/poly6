"use client";

import { motion } from "framer-motion";
import { Loader2, Lock, User, Zap } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { toast } from "sonner";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";

export default function LoginPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const { setAuth } = useAppStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);

    try {
      const result = await apiFetch<{ token: string; user_id: number; username: string }>(
        "/auth/login",
        {
          method: "POST",
          body: JSON.stringify({ username, password }),
        }
      );
      setAuth(result.token, { id: result.user_id, email: "", username: result.username });
      toast.success("Sikeres bejelentkezés!");
      router.push("/");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Hibás felhasználónév vagy jelszó");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div
      style={{
        minHeight: "100vh",
        background: "#0b0b0f",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        position: "relative",
        overflow: "hidden",
      }}
    >
      {/* Ambient glow */}
      <div
        className="ambient-glow ambient-glow-primary"
        style={{ width: 800, height: 800, top: "20%", left: "30%" }}
      />
      <div
        className="ambient-glow ambient-glow-green"
        style={{ width: 400, height: 400, top: "60%", right: "20%" }}
      />

      {/* Login card */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        className="glass-card"
        style={{ padding: "2rem", width: "100%", maxWidth: 400 }}
      >
        {/* Logo */}
        <div
          style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "2rem" }}
        >
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
            <Zap size={24} style={{ color: "#6366f1" }} />
          </div>
          <div>
            <h1 style={{ fontWeight: 700, fontSize: 24, color: "#fafafa" }}>PolyTrade</h1>
            <span style={{ fontSize: 12, color: "#71717a" }}>Command Center V2</span>
          </div>
        </div>

        {/* Title */}
        <h2 style={{ fontSize: 20, fontWeight: 600, color: "#fafafa", marginBottom: "0.5rem" }}>
          Bejelentkezés
        </h2>
        <p style={{ color: "#71717a", fontSize: 14, marginBottom: "1.5rem" }}>
          Add meg az adataidat a folytatáshoz
        </p>

        {/* Form */}
        <form
          onSubmit={handleSubmit}
          style={{ display: "flex", flexDirection: "column", gap: "1rem" }}
        >
          {/* Username */}
          <div>
            <label
              htmlFor="username"
              style={{ fontSize: 14, color: "#a1a1aa", marginBottom: "0.5rem", display: "block" }}
            >
              Felhasználónév
            </label>
            <div style={{ position: "relative" }}>
              <User
                size={20}
                style={{
                  color: "#71717a",
                  position: "absolute",
                  left: 12,
                  top: "50%",
                  transform: "translateY(-50%)",
                }}
              />
              <input
                id="username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="input"
                style={{ paddingLeft: 40 }}
                placeholder="username"
                required
                minLength={3}
              />
            </div>
          </div>

          {/* Password */}
          <div>
            <label
              htmlFor="password"
              style={{ fontSize: 14, color: "#a1a1aa", marginBottom: "0.5rem", display: "block" }}
            >
              Jelszó
            </label>
            <div style={{ position: "relative" }}>
              <Lock
                size={20}
                style={{
                  color: "#71717a",
                  position: "absolute",
                  left: 12,
                  top: "50%",
                  transform: "translateY(-50%)",
                }}
              />
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="input"
                style={{ paddingLeft: 40 }}
                placeholder="••••••••"
                required
                minLength={6}
              />
            </div>
          </div>

          {/* Submit button */}
          <button
            type="submit"
            disabled={isLoading}
            className="btn-primary"
            style={{
              width: "100%",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "0.5rem",
            }}
          >
            {isLoading ? <Loader2 size={20} className="animate-spin" /> : "Bejelentkezés"}
          </button>
        </form>

        {/* Register link */}
        <p style={{ textAlign: "center", color: "#71717a", fontSize: 14, marginTop: "1.5rem" }}>
          Még nincs fiók?{" "}
          <Link href="/register" style={{ color: "#6366f1" }}>
            Regisztrálj
          </Link>
        </p>

        {/* Demo mode */}
        <div
          style={{
            marginTop: "1.5rem",
            padding: "1rem",
            borderRadius: 8,
            background: "rgba(20, 20, 28, 0.6)",
            border: "1px solid rgba(255, 255, 255, 0.08)",
          }}
        >
          <p style={{ fontSize: 12, color: "#71717a", marginBottom: "0.5rem" }}>
            Demo mód (fejlesztés)
          </p>
          <button
            type="button"
            onClick={() => {
              // Create a demo user in the backend first
              apiFetch<{ token: string; user_id: number; username: string }>("/auth/register", {
                method: "POST",
                body: JSON.stringify({ username: "demo_user", password: "demo123" }),
              })
                .then((result) => {
                  setAuth(result.token, {
                    id: result.user_id,
                    email: "",
                    username: result.username,
                  });
                  toast.success("Demo fiók létrehozva!");
                  router.push("/");
                })
                .catch(() => {
                  // If user exists, try to login
                  apiFetch<{ token: string; user_id: number; username: string }>("/auth/login", {
                    method: "POST",
                    body: JSON.stringify({ username: "demo_user", password: "demo123" }),
                  })
                    .then((result) => {
                      setAuth(result.token, {
                        id: result.user_id,
                        email: "",
                        username: result.username,
                      });
                      toast.success("Demo bejelentkezés sikeres!");
                      router.push("/");
                    })
                    .catch(() => {
                      toast.error("Demo bejelentkezés sikertelen");
                    });
                });
            }}
            style={{
              fontSize: 14,
              color: "#6366f1",
              background: "none",
              border: "none",
              cursor: "pointer",
            }}
          >
            Bejelentkezés demo módban →
          </button>
        </div>
      </motion.div>
    </div>
  );
}
