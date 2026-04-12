import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";
import { createClient } from "redis";

export async function POST(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  // Verify schedule belongs to user
  const { rows } = await pool.query(
    "SELECT id FROM schedules WHERE id = $1 AND (user_id = $2 OR user_id = '00000000-0000-0000-0000-000000000000')",
    [id, userId]
  );
  if (rows.length === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  // Set Redis trigger key for the agent to pick up
  const redisUrl = process.env.REDIS_URL || "redis://127.0.0.1:6379";
  const redis = createClient({ url: redisUrl });
  try {
    await redis.connect();
    await redis.set(`schedule_trigger:${id}`, "1", { EX: 300 }); // 5min TTL
    await redis.quit();
  } catch (e) {
    console.error("Redis trigger error:", e);
    return NextResponse.json({ error: "Failed to trigger schedule" }, { status: 500 });
  }

  return NextResponse.json({ ok: true, message: "Schedule triggered" });
}
