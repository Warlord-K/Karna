export function register() {
  if (process.env.NEXT_RUNTIME === "nodejs") {
    const required = ["DATABASE_URL", "AUTH_SECRET"] as const;
    const missing = required.filter((k) => !process.env[k]);

    if (missing.length > 0) {
      console.error(
        `\n[FATAL] Missing required environment variables: ${missing.join(", ")}\n` +
          `Copy .env.example to .env, fill in the values, and restart.\n`
      );
      process.exit(1);
    }

    if (process.env.AUTH_DISABLED === "true") {
      console.warn("[WARN] AUTH_DISABLED=true — all routes accessible without auth");
    }
  }
}
