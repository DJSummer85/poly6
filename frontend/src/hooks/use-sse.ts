"use client";

import { useCallback, useEffect, useRef } from "react";
import { createSSEConnection, useAppStore } from "@/store";

// Module-level singleton to prevent multiple SSE connections
let sharedEventSource: EventSource | null = null;
let listenerCount = 0;

export function useSSE() {
  const prevEventStartTimeRef = useRef<number>(0);
  const lastConfirmedStartPriceRef = useRef<number>(0);
  // Use a ref to avoid re-creating setupConnection when systemStatus changes
  const systemStatusRef = useRef(useAppStore.getState().systemStatus);

  const {
    setMarketData,
    addMarketResult,
    addLog,
    updateBot,
    setSystemStatus,
    setLatency,
    addBotActivity,
  } = useAppStore();

  // Keep ref in sync
  useEffect(() => {
    const unsub = useAppStore.subscribe((s) => {
      systemStatusRef.current = s.systemStatus;
    });
    return unsub;
  }, []);

  const setupConnection = useCallback(() => {
    // Singleton: reuse existing connection
    if (sharedEventSource && sharedEventSource.readyState !== EventSource.CLOSED) {
      listenerCount++;
      return sharedEventSource;
    }

    // Create new connection
    const eventSource = createSSEConnection((event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === "connected") {
          console.log("SSE connected");
        }
      } catch {
        // Named events come through onmessage with event data
      }
    });

    eventSource.addEventListener("connected", () => {
      console.log("SSE connected event received");
    });

    eventSource.addEventListener("market", (e: MessageEvent) => {
      try {
        // Measure SSE event processing latency: from event arrival to state update
        const t0 = performance.now();
        const data = JSON.parse(e.data);
        // Check if start_price is explicitly present in the event (backend sends it only when captured)
        const hasStartPrice = data.start_price !== undefined || data.price_to_beat !== undefined;
        const newStartPrice = data.start_price || data.price_to_beat || 0;
        const eventStartTime = data.event_start_time || 0;

        // Detect new market start (event_start_time changed)
        if (
          eventStartTime > 0 &&
          prevEventStartTimeRef.current > 0 &&
          eventStartTime !== prevEventStartTimeRef.current
        ) {
          addMarketResult({
            endTime: Date.now(),
            targetPrice: lastConfirmedStartPriceRef.current,
            finalPrice: data.btc_price,
            delta: data.btc_price - lastConfirmedStartPriceRef.current,
            duration: 300,
          });
        }

        // Track event start time
        if (eventStartTime > 0) {
          if (
            prevEventStartTimeRef.current === 0 ||
            eventStartTime !== prevEventStartTimeRef.current
          ) {
            prevEventStartTimeRef.current = eventStartTime;
          }
        }

        // Only update start price if we have a valid new value
        if (newStartPrice > 0) {
          lastConfirmedStartPriceRef.current = newStartPrice;
        }

        // Batch all market data updates into single set() call
        const updates: Record<string, number | string> = {};
        if (data.btc_price !== undefined) updates.btcPrice = data.btc_price;
        // Only include startPrice and priceDelta when start_price has been captured (hasStartPrice)
        // This prevents flickering during market transitions
        if (hasStartPrice && newStartPrice > 0) {
          updates.startPrice = newStartPrice;
          updates.priceDelta = data.price_delta ?? data.btc_price - newStartPrice;
        } else if (hasStartPrice && lastConfirmedStartPriceRef.current > 0) {
          // Only show delta if we have a confirmed start price AND start_price was in the event
          updates.priceDelta = data.btc_price - lastConfirmedStartPriceRef.current;
        }
        if (hasStartPrice && (data.price_to_beat || data.beat_price))
          updates.beatPrice = data.price_to_beat || data.beat_price;
        if (data.yes !== undefined) updates.yesPrice = data.yes;
        if (data.no !== undefined) updates.noPrice = data.no;
        if (data.yes_price !== undefined) updates.yesPrice = data.yes_price;
        if (data.no_price !== undefined) updates.noPrice = data.no_price;
        if (data.market_question) updates.marketQuestion = data.market_question;
        if (data.time_remaining !== undefined) updates.timeRemaining = data.time_remaining;
        if (data.volume !== undefined) updates.volume = data.volume;
        if (data.api_latency !== undefined) updates.apiLatency = data.api_latency;

        if (Object.keys(updates).length > 0) {
          setMarketData(updates);
        }

        // Measure SSE event processing latency (event arrival → state update scheduled)
        // This captures JSON parse + data processing + Zustand set() call overhead
        // For the full end-to-end pipeline (including network), this represents the
        // frontend-side contribution. Network + backend processing add ~10-30ms on localhost.
        const t1 = performance.now();
        const latencyMs = t1 - t0;
        if (latencyMs >= 0.01) {
          setLatency(latencyMs);
        }
      } catch (err) {
        console.error("Failed to parse market event:", err);
      }
    });

    eventSource.addEventListener("status", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        const current = systemStatusRef.current;
        setSystemStatus({
          bots_running: data.running_bots,
          bots_total: current?.bots_total ?? data.running_bots,
          total_pnl: data.total_pnl,
          active_positions: current?.active_positions ?? 0,
          binance_connected: current?.binance_connected ?? true,
          last_update: Date.now(),
        });
      } catch (err) {
        console.error("Failed to parse status event:", err);
      }
    });

    // Handle bot lifecycle and trading events
    eventSource.addEventListener("bot", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);

        switch (data.type) {
          case "session_started": {
            addLog({
              bot_id: data.bot_id,
              bot_name: data.bot_name || `Bot ${data.bot_id}`,
              message: `Session started (ID: ${data.session_id})`,
              timestamp: Date.now(),
              level: "success",
            });
            updateBot(data.bot_id, { status: "running" });
            break;
          }

          case "session_ended": {
            addLog({
              bot_id: data.bot_id,
              bot_name: `Bot ${data.bot_id}`,
              message: `Session ended. PnL: $${data.total_pnl.toFixed(2)}`,
              timestamp: Date.now(),
              level: "info",
            });
            updateBot(data.bot_id, { status: "stopped" });
            break;
          }

          case "trade_decision": {
            const outcomeText = data.outcome === "YES" ? "UP" : "DOWN";
            const confidencePct = (data.confidence * 100).toFixed(0);
            addLog({
              bot_id: data.bot_id,
              bot_name: `Bot ${data.bot_id}`,
              message: `Decision: ${outcomeText} @ $${data.bet_size.toFixed(2)} (confidence: ${confidencePct}%). ${data.reason}`,
              timestamp: Date.now(),
              level: data.confidence > 0.7 ? "success" : "info",
            });
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "trade_decision",
              timestamp: Date.now(),
              data: {
                outcome: data.outcome,
                betSize: data.bet_size,
                confidence: data.confidence,
                reason: data.reason,
              },
            });
            break;
          }

          case "order_executed": {
            addLog({
              bot_id: data.bot_id,
              bot_name: `Bot ${data.bot_id}`,
              message: `Order placed: ${data.order_id}`,
              timestamp: Date.now(),
              level: "success",
            });
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "order_executed",
              timestamp: Date.now(),
              data: { orderId: data.order_id },
            });
            break;
          }

          case "balance_updated": {
            break;
          }

          case "error": {
            addLog({
              bot_id: data.bot_id,
              bot_name: `Bot ${data.bot_id}`,
              message: data.message,
              timestamp: Date.now(),
              level: "error",
            });
            updateBot(data.bot_id, { status: "error" });
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "error",
              timestamp: Date.now(),
              data: { message: data.message },
            });
            break;
          }

          case "market_transition": {
            addLog({
              bot_id: 0,
              bot_name: "System",
              message: `Market transition: ${data.new_market_slug}`,
              timestamp: Date.now(),
              level: "info",
            });
            break;
          }

          case "scanning": {
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "scanning",
              timestamp: Date.now(),
              data: { market: data.market_slug },
            });
            break;
          }

          case "evaluating": {
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "evaluating",
              timestamp: Date.now(),
              data: { strategy: data.strategy, confidence: data.confidence },
            });
            break;
          }

          case "position_update": {
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "position_update",
              timestamp: Date.now(),
              data: {
                side: data.side,
                size: data.size,
                price: data.price,
                unrealizedPnl: data.unrealized_pnl,
              },
            });
            break;
          }

          case "trade_result": {
            addBotActivity(data.bot_id, {
              botId: data.bot_id,
              type: "trade_result",
              timestamp: Date.now(),
              data: { won: data.won, pnl: data.pnl },
            });
            break;
          }
        }
      } catch (err) {
        console.error("Failed to parse bot event:", err);
      }
    });

    eventSource.onerror = () => {
      console.warn("SSE connection error, reconnecting...");
    };

    sharedEventSource = eventSource;
    listenerCount = 1;
    return eventSource;
  }, [
    setMarketData,
    addMarketResult,
    addLog,
    updateBot,
    setSystemStatus,
    setLatency,
    addBotActivity,
  ]);

  const disconnect = useCallback(() => {
    listenerCount--;
    if (listenerCount <= 0 && sharedEventSource) {
      sharedEventSource.close();
      sharedEventSource = null;
      listenerCount = 0;
    }
  }, []);

  useEffect(() => {
    setupConnection();
    return () => disconnect();
  }, [setupConnection, disconnect]);

  return {
    connect: setupConnection,
    disconnect,
  };
}
