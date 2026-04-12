import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  const { rows } = await pool.query(
    "SELECT * FROM schedules WHERE id = $1 AND (user_id = $2 OR user_id = '00000000-0000-0000-0000-000000000000')",
    [id, userId]
  );

  if (rows.length === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  return NextResponse.json(rows[0]);
}

export async function PATCH(req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;
  const updates = await req.json();

  const allowedFields = [
    "name", "prompt", "repos", "cron_expression", "run_at",
    "skills", "mcp_servers", "max_open_tasks", "task_prefix",
    "priority", "cli", "model", "enabled",
  ];

  const setClauses: string[] = [];
  const values: unknown[] = [];
  let paramIndex = 1;

  for (const [key, value] of Object.entries(updates)) {
    if (allowedFields.includes(key)) {
      setClauses.push(`"${key}" = $${paramIndex}`);
      values.push(value);
      paramIndex++;
    }
  }

  if (setClauses.length === 0) {
    return NextResponse.json({ error: "No valid fields to update" }, { status: 400 });
  }

  values.push(id, userId);

  const { rowCount } = await pool.query(
    `UPDATE schedules SET ${setClauses.join(", ")}
     WHERE id = $${paramIndex} AND (user_id = $${paramIndex + 1} OR user_id = '00000000-0000-0000-0000-000000000000')`,
    values
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  return NextResponse.json({ ok: true });
}

export async function DELETE(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  const { rowCount } = await pool.query(
    "DELETE FROM schedules WHERE id = $1 AND (user_id = $2 OR user_id = '00000000-0000-0000-0000-000000000000')",
    [id, userId]
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  return NextResponse.json({ ok: true });
}
