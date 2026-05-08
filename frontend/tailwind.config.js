/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
    "./components/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        dark: "#0b0b0f",
        background: "var(--bg-dark)",
        surface: "var(--bg-surface)",
        "surface-2": "var(--bg-surface-2)",
        border: "var(--border-color)",
        "border-hover": "var(--border-hover)",
        primary: "var(--primary)",
        "primary-hover": "var(--primary-hover)",
        "primary-muted": "var(--primary-muted)",
        btc: "var(--btc)",
        "btc-muted": "var(--btc-muted)",
        "neon-green": "var(--green)",
        "neon-green-muted": "var(--green-muted)",
        "neon-red": "var(--red)",
        "neon-red-muted": "var(--red-muted)",
        "neon-blue": "var(--blue)",
        "neon-blue-muted": "var(--blue-muted)",
        text: "var(--text-primary)",
        "text-primary": "var(--text-primary)",
        "text-secondary": "var(--text-secondary)",
        "text-muted": "var(--text-muted)",
      },
      opacity: {
        3: "0.03",
        8: "0.08",
      },
    },
  },
  plugins: [],
};
