import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string; runId: string }> }
) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id, runId } = await params;

  // Verify schedule belongs to user
  const { rows: scheduleRows } = await pool.query(
    "SELECT id FROM schedules WHERE id = $1 AND user_id = $2",
    [id, userId]
  );
  if (scheduleRows.length === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const { rows } = await pool.query(
    `SELECT * FROM scheduled_run_logs
     WHERE run_id = $1
     ORDER BY created_at ASC`,
    [runId]
  );

  return NextResponse.json(rows);
}
