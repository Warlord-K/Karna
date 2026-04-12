import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  // Verify schedule belongs to user
  const { rows: scheduleRows } = await pool.query(
    "SELECT id FROM schedules WHERE id = $1 AND (user_id = $2 OR user_id = '00000000-0000-0000-0000-000000000000')",
    [id, userId]
  );
  if (scheduleRows.length === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const { rows } = await pool.query(
    `SELECT id, schedule_id, status, started_at, completed_at, summary_markdown,
            coalesce(array_length(tasks_created, 1), 0) as task_count, cost_usd, created_at
     FROM scheduled_runs
     WHERE schedule_id = $1
     ORDER BY started_at DESC
     LIMIT 50`,
    [id]
  );

  return NextResponse.json(rows);
}
