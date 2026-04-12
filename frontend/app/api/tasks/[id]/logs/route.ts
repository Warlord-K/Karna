import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  // Verify the task belongs to the user
  const { rowCount } = await pool.query(
    "SELECT 1 FROM agent_tasks WHERE id = $1 AND user_id = $2",
    [id, userId]
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const { rows } = await pool.query(
    "SELECT * FROM agent_logs WHERE task_id = $1 ORDER BY created_at ASC",
    [id]
  );

  return NextResponse.json(rows);
}
