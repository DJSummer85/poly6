"use client";

import { useCallback, useEffect, useRef } from "react";

interface TradingViewWidgetProps {
  symbol?: string;
  interval?: string;
  height?: number;
}

declare global {
  interface Window {
    TradingView: {
      widget: new (config: Record<string, unknown>) => unknown;
    };
  }
}

export function TradingViewWidget({
  symbol = "BINANCE:BTCUSDT",
  interval = "5",
  height = 400,
}: TradingViewWidgetProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const widgetRef = useRef<unknown>(null);
  const scriptRef = useRef<HTMLScriptElement | null>(null);
  const isInitialized = useRef(false);

  const cleanup = useCallback(() => {
    if (widgetRef.current) {
      try {
        const widget = widgetRef.current as { remove?: () => void };
        if (typeof widget.remove === "function") {
          widget.remove();
        }
      } catch {
        // Ignore cleanup errors
      }
      widgetRef.current = null;
    }

    // Safe DOM clearing - remove children one by one
    if (containerRef.current) {
      while (containerRef.current.firstChild) {
        containerRef.current.removeChild(containerRef.current.firstChild);
      }
    }

    if (scriptRef.current?.parentNode) {
      scriptRef.current.remove();
      scriptRef.current = null;
    }

    isInitialized.current = false;
  }, []);

  useEffect(() => {
    if (!containerRef.current || isInitialized.current) return;

    cleanup();

    const script = document.createElement("script");
    script.src = "https://s3.tradingview.com/tv.js";
    script.async = true;

    script.onload = () => {
      if (window.TradingView && containerRef.current) {
        try {
          if (typeof window.TradingView.widget === "function") {
            widgetRef.current = new window.TradingView.widget({
              autosize: true,
              symbol: symbol,
              interval: interval,
              timezone: "Etc/UTC",
              theme: "dark",
              style: "1",
              locale: "en",
              toolbar_bg: "#0b0b0f",
              enable_publishing: false,
              hide_top_toolbar: false,
              hide_legend: false,
              save_image: false,
              container_id: containerRef.current.id,
              backgroundColor: "#0b0b0f",
              gridColor: "rgba(255,255,255,0.05)",
              textColor: "#a1a1aa",
              hide_volume: false,
              disabled_features: [
                "header_symbol_search",
                "header_compare",
                "header_chart_type",
                "header_screenshot",
                "header_fullscreen_button",
                "create_volume_indicator_by_default",
              ],
              enabled_features: ["hide_left_toolbar_by_default"],
              overrides: {
                "mainSeriesProperties.candleStyle.upColor": "#22c55e",
                "mainSeriesProperties.candleStyle.downColor": "#ef4444",
                "mainSeriesProperties.candleStyle.borderUpColor": "#22c55e",
                "mainSeriesProperties.candleStyle.borderDownColor": "#ef4444",
                "mainSeriesProperties.candleStyle.wickUpColor": "#22c55e",
                "mainSeriesProperties.candleStyle.wickDownColor": "#ef4444",
                "paneProperties.background": "#0b0b0f",
                "paneProperties.vertGridProperties.color": "rgba(255,255,255,0.03)",
                "paneProperties.horzGridProperties.color": "rgba(255,255,255,0.03)",
                "scalesProperties.textColor": "#71717a",
              },
            });
            isInitialized.current = true;
          }
        } catch (e) {
          console.error("TradingView widget init error:", e);
        }
      }
    };

    scriptRef.current = script;
    document.head.appendChild(script);

    return cleanup;
  }, [symbol, interval, cleanup]);

  return (
    <div
      id={`tv-chart-${symbol.replace(/[^a-zA-Z0-9]/g, "-")}`}
      ref={containerRef}
      style={{ height: `${height}px`, width: "100%" }}
    />
  );
}
