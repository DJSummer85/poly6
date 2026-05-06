"use client";

import { motion } from "framer-motion";
import { Loader2, Lock, User, Zap } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { toast } from "sonner";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";

export default function RegisterPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const { setAuth } = useAppStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (password !== confirmPassword) {
      toast.error("A jelszavak nem egyeznek");
      return;
    }

    if (username.length < 3) {
      toast.error("A felhasználónév minimum 3 karakter");
      return;
    }

    if (password.length < 6) {
      toast.error("A jelszó minimum 6 karakter");
      return;
    }

    setIsLoading(true);

    try {
      const result = await apiFetch<{ token: string; user_id: number; username: string }>(
        "/auth/register",
        {
          method: "POST",
          body: JSON.stringify({ username, password }),
        }
      );
      setAuth(result.token, { id: result.user_id, email: "", username: result.username });
      toast.success("Sikeres regisztráció!");
      router.push("/");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Regisztráció sikertelen");
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
        className="ambient-glow ambient-glow-blue"
        style={{ width: 400, height: 400, top: "60%", right: "20%" }}
      />

      {/* Register card */}
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
          Regisztráció
        </h2>
        <p style={{ color: "#71717a", fontSize: 14, marginBottom: "1.5rem" }}>
          Hozz létre új fiókot a trading bot használatához
        </p>

        {/* Form */}
        <form
          onSubmit={handleSubmit}
          style={{ display: "flex", flexDirection: "column", gap: "1rem" }}
        >
          {/* Username */}
          <div>
            <label
              htmlFor="register-username"
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
                id="register-username"
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
              htmlFor="register-password"
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
                id="register-password"
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

          {/* Confirm Password */}
          <div>
            <label
              htmlFor="register-confirm-password"
              style={{ fontSize: 14, color: "#a1a1aa", marginBottom: "0.5rem", display: "block" }}
            >
              Jelszó megerősítése
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
                id="register-confirm-password"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="input"
                style={{ paddingLeft: 40 }}
                placeholder="••••••••"
                required
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
            {isLoading ? <Loader2 size={20} className="animate-spin" /> : "Regisztráció"}
          </button>
        </form>

        {/* Login link */}
        <p style={{ textAlign: "center", color: "#71717a", fontSize: 14, marginTop: "1.5rem" }}>
          Már van fiók?{" "}
          <Link href="/login" style={{ color: "#6366f1" }}>
            Bejelentkezés
          </Link>
        </p>
      </motion.div>
    </div>
  );
}
