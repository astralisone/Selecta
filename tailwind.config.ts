import type { Config } from "tailwindcss";
import animate from "tailwindcss-animate";

export default {
  darkMode: "class",
  content: ["./index.html", "./ui/src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Bassministry Iris (primary)
        iris: {
          50: "#EEEFFB",
          100: "#DEDFF7",
          200: "#BEC0EF",
          300: "#9EA0E7",
          400: "#7C7EE5",
          500: "#575AD0",
          600: "#4749A6",
          700: "#33357A",
          800: "#20214F",
          900: "#101125",
        },
        // Deep blue (secondary)
        deep: {
          50: "#E8EEF4",
          100: "#C6D3E0",
          200: "#A4B8CC",
          300: "#7C97B4",
          400: "#476A94",
          500: "#304D6D",
          600: "#263D58",
          700: "#1C2D41",
          800: "#131F2C",
          900: "#0A1118",
        },
        // Gold (accent)
        gold: {
          50: "#FFF7E1",
          100: "#FFE9B3",
          200: "#FFD156",
          300: "#F5B71C",
          400: "#D99704",
          500: "#A87703",
          600: "#7C5702",
          700: "#513901",
          800: "#271B01",
        },
        // Dark surfaces
        bg: {
          DEFAULT: "#01020F",
          card: "#06071A",
          elevated: "#0D0F28",
          deep: "#00010A",
        },
        // UI semantic tokens so shadcn-like patterns work without the registry
        border: "rgb(255 255 255 / 0.08)",
        input: "rgb(255 255 255 / 0.08)",
        ring: "#575AD0",
        foreground: "#E1E1E3",
        muted: {
          DEFAULT: "rgb(255 255 255 / 0.04)",
          foreground: "#9EA4B2",
        },
      },
      fontFamily: {
        sans: [
          '"DM Sans"',
          "ui-sans-serif",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          '"JetBrains Mono"',
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "monospace",
        ],
      },
      letterSpacing: {
        tightest: "-0.05em",
      },
      borderRadius: {
        lg: "14px",
        md: "10px",
        sm: "6px",
      },
      boxShadow: {
        "glow-iris": "0 0 24px rgba(87, 90, 208, 0.5)",
        "glow-iris-sm": "0 0 12px rgba(87, 90, 208, 0.35)",
        "glow-gold": "0 0 20px rgba(245, 183, 28, 0.45)",
        glass: "0 4px 30px rgba(0, 0, 0, 0.35)",
        "inset-hairline": "inset 0 0 0 1px rgba(255,255,255,0.06)",
      },
      backdropBlur: {
        xs: "4px",
      },
      keyframes: {
        "fade-in": {
          "0%": { opacity: "0", transform: "translateY(4px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        shimmer: {
          "0%": { backgroundPosition: "-400px 0" },
          "100%": { backgroundPosition: "400px 0" },
        },
        "glow-pulse": {
          "0%, 100%": {
            boxShadow: "0 0 18px rgba(87, 90, 208, 0.35)",
          },
          "50%": {
            boxShadow: "0 0 32px rgba(87, 90, 208, 0.6)",
          },
        },
      },
      animation: {
        "fade-in": "fade-in 0.3s ease-out",
        shimmer: "shimmer 2s linear infinite",
        "glow-pulse": "glow-pulse 3s ease-in-out infinite",
      },
      backgroundImage: {
        "grid-faint":
          "linear-gradient(rgba(255,255,255,0.03) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.03) 1px, transparent 1px)",
        "iris-gradient":
          "linear-gradient(135deg, rgba(87,90,208,0.18) 0%, rgba(48,77,109,0.12) 100%)",
      },
    },
  },
  plugins: [animate],
} satisfies Config;
