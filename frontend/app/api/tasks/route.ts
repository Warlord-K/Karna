import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET() {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { rows } = await pool.query(
    `SELECT * FROM agent_tasks
     WHERE user_id = $1
     ORDER BY
       CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END,
       created_at ASC`,
    [userId]
  );

  return NextResponse.json(rows);
}

export async function POST(req: NextRequest) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { title, description, repo, priority, cli, model } = await req.json();

  if (!title?.trim()) {
    return NextResponse.json({ error: "title is required" }, { status: 400 });
  }

  try {
    const { rows } = await pool.query(
      `INSERT INTO agent_tasks (user_id, title, description, repo, priority, position, cli, model)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
       RETURNING *`,
      [userId, title.trim(), description || null, repo || null, priority || "medium", Date.now(), cli || null, model || null]
    );

    return NextResponse.json(rows[0], { status: 201 });
  } catch (error: any) {
    console.error("error:", error);
    if (error?.code === "23503" && error?.constraint === "agent_tasks_user_id_fkey") {
      return NextResponse.json(
        { error: "User not found. Try signing out and back in." },
        { status: 401 }
      );
    }
    return NextResponse.json({ error: "Failed to create task" }, { status: 500 });
  }
}
