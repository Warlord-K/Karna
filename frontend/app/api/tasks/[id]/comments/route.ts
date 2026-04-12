import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function POST(req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;
  const { message } = await req.json();

  if (!message || typeof message !== "string" || !message.trim()) {
    return NextResponse.json({ error: "Message is required" }, { status: 400 });
  }

  // Verify the task belongs to the user
  const { rows: taskRows } = await pool.query(
    "SELECT status FROM agent_tasks WHERE id = $1 AND user_id = $2",
    [id, userId]
  );

  if (taskRows.length === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const status = taskRows[0].status;
  const trimmed = message.trim();

  // Insert comment as a log entry
  const { rows: logRows } = await pool.query(
    `INSERT INTO agent_logs (task_id, phase, message, log_type)
     VALUES ($1, 'comment', $2, 'comment')
     RETURNING *`,
    [id, trimmed]
  );

  // Set feedback on task so the agent picks it up
  // Status transitions: review → in_progress, plan_review → planning
  if (status === "review") {
    await pool.query(
      `UPDATE agent_tasks SET feedback = $1, status = 'in_progress' WHERE id = $2`,
      [trimmed, id]
    );
  } else if (status === "plan_review") {
    await pool.query(
      `UPDATE agent_tasks SET feedback = $1, status = 'planning' WHERE id = $2`,
      [trimmed, id]
    );
  } else {
    await pool.query(
      `UPDATE agent_tasks SET feedback = $1 WHERE id = $2`,
      [trimmed, id]
    );
  }

  return NextResponse.json(logRows[0]);
}
