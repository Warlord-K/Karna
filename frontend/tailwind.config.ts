import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: "class",
  content: [
    "./app/**/*.{ts,tsx}",
    "./components/**/*.{ts,tsx}",
    "./lib/**/*.{ts,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        // Warm semantic grays (sun-touched)
        gray: {
          1: "#131210",
          2: "#1a1917",
          3: "#22211e",
          4: "#2a2926",
          5: "#32312d",
          6: "#3d3b37",
          7: "#4d4b45",
          8: "#63615a",
          9: "#82807a",
          10: "#a09e97",
          11: "#bab8b1",
          12: "#edece8",
        },
        // Golden sun accent scale
        sun: {
          3: "#2b2213",
          4: "#3a2e18",
          5: "#4d3d1f",
          6: "#614d28",
          7: "#7a6233",
          8: "#b8923d",
          9: "#e5b847",
          10: "#f0c95e",
          11: "#f5d87a",
          12: "#faeab5",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      fontSize: {
        "2xs": ["0.6875rem", { lineHeight: "1rem" }],
      },
      boxShadow: {
        "card": "0 1px 3px rgba(0,0,0,0.25), 0 1px 2px rgba(0,0,0,0.15)",
        "card-hover": "0 4px 12px rgba(0,0,0,0.3), 0 1px 3px rgba(0,0,0,0.2)",
        "elevated": "0 8px 30px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.2)",
        "modal": "0 24px 80px rgba(0,0,0,0.55), 0 4px 16px rgba(0,0,0,0.3)",
      },
    },
  },
  plugins: [require("tailwindcss-animate"), require("@tailwindcss/typography")],
};

export default config;
