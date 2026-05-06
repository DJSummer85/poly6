"use client";

import { AlertTriangle, Crosshair, Loader2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useBots, useStartBot, useStopBot } from "@/hooks";
import { apiFetch } from "@/lib/utils";
import { useAppStore } from "@/store";
import type { Bot as BotType } from "@/types";
import { BotDetailCard } from "./bot-detail-card";
import { BotRow } from "./bot-row";

function LiveModeConfirmDialog({
  botName,
  onConfirm,
  onCancel,
}: {
  botName: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="rounded-xl border border-red-500/30 bg-zinc-900 p-6 shadow-2xl max-w-md w-full mx-4">
        <div className="flex items-center gap-3 mb-4">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-red-500/20">
            <AlertTriangle className="h-5 w-5 text-red-400" />
          </div>
          <div>
            <h3 className="text-base font-bold text-white">Valódi pénz!</h3>
            <p className="text-xs text-zinc-400">Live kereskedés megerősítése</p>
          </div>
          <button type="button" onClick={onCancel} className="ml-auto text-zinc-500 hover:text-zinc-300">
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-3 mb-4">
          <p className="text-sm text-red-300">
            ⚠️ A <strong>{botName}</strong> bot valódi USDC-t fog felhasználni a Polymarket tárcádból!
          </p>
          <p className="text-xs text-red-400 mt-1">
            A veszteségek valódiak. Csak akkor folytasd, ha tudod mit csinálsz!
          </p>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="flex-1 rounded-lg border border-zinc-700 bg-zinc-800 px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
          >
            Mégsem
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="flex-1 rounded-lg bg-red-500 px-4 py-2 text-sm font-bold text-white hover:bg-red-600 transition-colors"
          >
            Igen, indítás!
          </button>
        </div>
      </div>
    </div>
  );
}

export function BotSelector() {
  const { data: botsFromApi, isLoading, isFetching } = useBots();
  const { selectedBotIds, setSelectedBotIds, tradingMode } = useAppStore();
  const startBotMutation = useStartBot();
  const stopBotMutation = useStopBot();
  const [deletingId, setDeletingId] = useState<number | null>(null);
  const [confirmBot, setConfirmBot] = useState<{ id: number; name: string } | null>(null);

  const botList = botsFromApi ?? [];
  const isMutating = startBotMutation.isPending || stopBotMutation.isPending;

  // Filter bots by trading mode
  const filteredBots = botList;


  const startBot = (id: number) => {
    const bot = botList.find((b) => b.id === id);
    if (!bot) return;

    if (tradingMode === "live") {
      setConfirmBot({ id, name: bot.name });
      return;
    }

    startBotMutation.mutate({ id, initial_balance: 100 }, {
      onSuccess: () => toast.success("Bot elindítva"),
      onError: (err) => toast.error(err.message || "Hiba a bot indításakor"),
    });
  };

  const confirmStartLiveBot = () => {
    if (!confirmBot) return;
    startBotMutation.mutate({ id: confirmBot.id, initial_balance: 0 }, {
      onSuccess: () => {
        toast.success("Live bot elindítva!");
        setConfirmBot(null);
      },
      onError: (err) => {
        toast.error(err.message || "Hiba a bot indításakor");
        setConfirmBot(null);
      },
    });
  };

  const stopBot = (id: number) => {
    stopBotMutation.mutate(id, {
      onSuccess: () => toast.success("Bot leállítva"),
      onError: (err) => toast.error(err.message || "Hiba a bot leállításakor"),
    });
  };

  const handleToggle = (id: number) => {
    if (selectedBotIds.includes(id)) {
      setSelectedBotIds(selectedBotIds.filter((bid) => bid !== id));
    } else if (selectedBotIds.length < 2) {
      setSelectedBotIds([...selectedBotIds, id]);
    } else {
      toast.error("Maximum 2 bot választható ki egyszerre");
    }
  };

  const deleteBot = async (id: number) => {
    setDeletingId(id);
    try {
      await apiFetch(`/bots/${id}`, { method: "DELETE" });
      toast.success("Bot törölve!");
      if (selectedBotIds.includes(id)) {
        setSelectedBotIds(selectedBotIds.filter((bid) => bid !== id));
      }
    } catch {
      toast.error("Hiba a bot törlésekor");
    } finally {
      setDeletingId(null);
    }
  };

  const runningBots = filteredBots.filter((b) => b.status === "running");
  const idleBots = filteredBots.filter((b) => b.status !== "running");

  if (isLoading) {
    return (
      <div className="rounded-xl border border-white/8 bg-white/3 backdrop-blur-xl p-4">
        <div className="flex items-center gap-2">
          <div className="h-4 w-4 animate-spin rounded-full border-2 border-indigo-500 border-t-transparent" />
          <span className="text-sm text-zinc-500">Botok betöltése...</span>
        </div>
      </div>
    );
  }

  if (filteredBots.length === 0) {
    return (
      <div className="rounded-xl border border-white/8 bg-white/3 backdrop-blur-xl p-6 text-center">
        <Crosshair className="h-8 w-8 text-zinc-600 mx-auto mb-2" />
        <p className="text-sm font-medium text-zinc-400">
          Nincsenek {tradingMode === "demo" ? "demo" : "live"} botok
        </p>
        <p className="text-xs text-zinc-600 mt-1">
          {tradingMode === "live" ? (
            <>
              Hozz létre live botokat a{" "}
              <a href="/bots" className="text-indigo-400 hover:underline">Botok</a> oldalon
            </>
          ) : (
            <>
              Hozz létre egy botot a{" "}
              <a href="/bots" className="text-indigo-400 hover:underline">Botok</a> oldalon
            </>
          )}
        </p>
      </div>
    );
  }

  return (
    <>
      {confirmBot && (
        <LiveModeConfirmDialog
          botName={confirmBot.name}
          onConfirm={confirmStartLiveBot}
          onCancel={() => setConfirmBot(null)}
        />
      )}

      <div className="rounded-xl border border-white/8 bg-white/3 backdrop-blur-xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 shrink-0">
          <div className="flex items-center gap-2.5">
            <div className={`flex h-7 w-7 items-center justify-center rounded-lg ${tradingMode === "demo" ? "bg-indigo-500/15" : "bg-red-500/15"
              }`}>
              <Crosshair className={`h-3.5 w-3.5 ${tradingMode === "demo" ? "text-indigo-400" : "text-red-400"
                }`} />
            </div>
            <div className="flex items-center gap-1.5">
              <span className="text-sm font-semibold text-zinc-200">
                {tradingMode === "demo" ? "🎮 Demo" : "⚡ Live"} Botok
              </span>
              <span className="text-xs text-zinc-500">
                {runningBots.length > 0 && (
                  <span className="text-green-400">{runningBots.length} fut </span>
                )}
                <span>{filteredBots.length} összes</span>
              </span>
            </div>
          </div>
          {isFetching && !isLoading && <Loader2 className="h-3.5 w-3.5 text-zinc-600 animate-spin" />}
        </div>

        {/* Live mode warning */}
        {tradingMode === "live" && (
          <div className="mx-2 mb-2 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-1.5">
            <p className="text-[10px] font-bold text-red-400 uppercase tracking-wider">
              ⚡ Live mód – Valódi pénz!
            </p>
          </div>
        )}

        {/* Bot list */}
        <div
          className="px-2 pb-3 overflow-y-auto flex-1 space-y-0.5"
          style={{ maxHeight: "calc(100vh - 280px)" }}
        >
          {runningBots.length > 0 && (
            <>
              <div className="px-2 py-1">
                <span className="text-[9px] font-bold uppercase tracking-wider text-green-400/60">
                  Futó botok ({runningBots.length})
                </span>
              </div>
              {runningBots.map((bot) => (
                <BotRow
                  key={bot.id}
                  bot={bot}
                  isSelected={selectedBotIds.includes(bot.id)}
                  isRunning={true}
                  onToggle={() => handleToggle(bot.id)}
                  onStart={startBot}
                  onStop={stopBot}
                  onDelete={deleteBot}
                  isDeleting={deletingId === bot.id}
                  isMutating={isMutating}
                />
              ))}
            </>
          )}

          {idleBots.length > 0 && (
            <>
              <div className="px-2 py-1 mt-1">
                <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-500/60">
                  Tétlen ({idleBots.length})
                </span>
              </div>
              {idleBots.map((bot) => (
                <BotRow
                  key={bot.id}
                  bot={bot}
                  isSelected={selectedBotIds.includes(bot.id)}
                  isRunning={false}
                  onToggle={() => handleToggle(bot.id)}
                  onStart={startBot}
                  onStop={stopBot}
                  onDelete={deleteBot}
                  isDeleting={deletingId === bot.id}
                  isMutating={isMutating}
                />
              ))}
            </>
          )}

          {selectedBotIds.length > 0 && (
            <div className="mt-3 pt-3 border-t border-white/5 space-y-2">
              <span className="px-2 text-[9px] font-bold uppercase tracking-wider text-indigo-400/60">
                Részletek
              </span>
              {botList
                .filter((b: BotType) => selectedBotIds.includes(b.id))
                .map((bot) => (
                  <BotDetailCard
                    key={bot.id}
                    bot={bot}
                    isRunning={bot.status === "running"}
                    onStart={startBot}
                    onStop={stopBot}
                    isMutating={isMutating}
                  />
                ))}
            </div>
          )}
        </div>
      </div>
    </>
  );
}
