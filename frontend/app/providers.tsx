"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";
import { Toaster } from "sonner";

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 5 * 1000,
            refetchInterval: 10 * 1000,
          },
        },
      })
  );

  return (
    <QueryClientProvider client={queryClient}>
      {children}
      <Toaster
        position="bottom-right"
        theme="dark"
        toastOptions={{
          style: {
            background: "rgba(20, 20, 28, 0.85)",
            backdropFilter: "blur(20px)",
            border: "1px solid rgba(255, 255, 255, 0.1)",
            borderRadius: "12px",
            color: "#fafafa",
            boxShadow: "0 8px 32px rgba(0, 0, 0, 0.4)",
          },
          className: "toast-glass",
        }}
        icons={{
          success: (
            <div className="flex items-center justify-center w-5 h-5 rounded-full bg-green-500/20">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="#22c55e"
                strokeWidth="3"
                aria-label="Success"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </div>
          ),
          error: (
            <div className="flex items-center justify-center w-5 h-5 rounded-full bg-red-500/20">
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="#ef4444"
                strokeWidth="3"
                aria-label="Error"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </div>
          ),
        }}
      />
    </QueryClientProvider>
  );
}
