"use client";

import { motion } from "framer-motion";
import {
  Bot,
  ChevronLeft,
  ChevronRight,
  FlaskConical,
  History,
  LayoutDashboard,
  LineChart,
  Settings,
  Wallet,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useAppStore } from "@/store";

const navItems = [
  { href: "/", label: "Dashboard", icon: LayoutDashboard },
  { href: "/bots", label: "Bots", icon: Bot },
  { href: "/strategy-lab", label: "Strategy Lab", icon: FlaskConical },
  { href: "/markets", label: "Markets", icon: LineChart },
  { href: "/orders", label: "Orders", icon: History },
  { href: "/funding", label: "Funding", icon: Wallet },
  { href: "/settings", label: "Settings", icon: Settings },
];

interface SidebarProps {
  collapsed: boolean;
}

export function Sidebar({ collapsed }: SidebarProps) {
  const pathname = usePathname();
  const { toggleSidebar } = useAppStore();

  return (
    <motion.aside
      initial={false}
      animate={{ width: collapsed ? 64 : 220 }}
      transition={{ duration: 0.2, ease: "easeInOut" }}
      className="flex flex-col h-full bg-zinc-950/95 border-r border-white/8 backdrop-blur-xl relative"
    >
      {/* Logo Area */}
      <div className="flex items-center gap-3 px-4 py-4 border-b border-white/8">
        <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-indigo-500/15 flex-shrink-0">
          <LayoutDashboard className="h-5 w-5 text-indigo-500" />
        </div>
        {!collapsed && (
          <motion.span
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="text-base font-bold text-zinc-100"
          >
            PolyTrade
          </motion.span>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-3 mt-2">
        {navItems.map((item) => {
          const isActive = pathname === item.href;
          const Icon = item.icon;

          return (
            <Link
              key={item.href}
              href={item.href}
              className={`flex items-center gap-3 rounded-xl px-3 py-2.5 mb-1 cursor-pointer transition-all
              ${collapsed ? "justify-center" : "justify-start"}
              ${
                isActive
                  ? "bg-indigo-500/15 border border-indigo-500/30 text-indigo-400"
                  : "text-zinc-500 hover:bg-white/5 hover:text-zinc-200"
              }`}
            >
              <Icon className="h-5 w-5 flex-shrink-0" />
              {!collapsed && (
                <motion.span
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  className="text-sm font-medium"
                >
                  {item.label}
                </motion.span>
              )}
            </Link>
          );
        })}
      </nav>

      {/* Emergency Stop */}
      {!collapsed && (
        <div className="border-t border-white/8 p-3 pb-4">
          <button type="button" className="btn-emergency w-full">
            Emergency Stop
          </button>
        </div>
      )}

      {/* Collapse toggle */}
      <button
        type="button"
        onClick={toggleSidebar}
        className="absolute -right-3 top-4 flex h-6 w-6 items-center justify-center rounded-full bg-zinc-900/60 border border-white/8 text-zinc-400 hover:text-zinc-200 cursor-pointer z-10 transition-colors"
      >
        {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
      </button>
    </motion.aside>
  );
}
