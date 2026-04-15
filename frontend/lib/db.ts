import { Pool } from "pg";

// Use globalThis to survive Next.js module re-instantiation in standalone mode.
// Without this, multiple Pool instances can be created across different bundles,
// each holding connections against Postgres.
const globalForPg = globalThis as unknown as { _pgPool?: Pool };

export function getPool(): Pool {
  if (!globalForPg._pgPool) {
    globalForPg._pgPool = new Pool({
      connectionString: process.env.DATABASE_URL,
      max: 10,
      connectionTimeoutMillis: 5000,
      idleTimeoutMillis: 30000,
    });
  }
  return globalForPg._pgPool;
}

// Backwards-compat: eager pool for existing imports
export const pool = new Proxy({} as Pool, {
  get(_, prop) {
    return (getPool() as any)[prop];
  },
});
