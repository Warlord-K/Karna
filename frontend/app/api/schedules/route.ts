import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET() {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { rows } = await pool.query(
    `SELECT s.*,
       (SELECT row_to_json(r) FROM (
         SELECT id, status, started_at, completed_at, summary_markdown,
                coalesce(array_length(tasks_created, 1), 0) as task_count, cost_usd
         FROM scheduled_runs WHERE schedule_id = s.id
         ORDER BY started_at DESC LIMIT 1
       ) r) as last_run
     FROM schedules s
     WHERE (s.user_id = $1 OR s.user_id = '00000000-0000-0000-0000-000000000000')
     ORDER BY s.created_at DESC`,
    [userId]
  );

  return NextResponse.json(rows);
}

export async function POST(req: NextRequest) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const body = await req.json();
  const {
    name, prompt, repos, cron_expression, run_at,
    skills, mcp_servers, max_open_tasks, task_prefix,
    priority, cli, model,
  } = body;

  if (!name?.trim()) {
    return NextResponse.json({ error: "name is required" }, { status: 400 });
  }
  if (!prompt?.trim()) {
    return NextResponse.json({ error: "prompt is required" }, { status: 400 });
  }
  if (!cron_expression && !run_at) {
    return NextResponse.json({ error: "Either cron_expression or run_at is required" }, { status: 400 });
  }

  const { rows } = await pool.query(
    `INSERT INTO schedules (
       user_id, name, prompt, repos, cron_expression, run_at,
       skills, mcp_servers, max_open_tasks, task_prefix,
       priority, cli, model
     ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
     RETURNING *`,
    [
      userId,
      name.trim(),
      prompt.trim(),
      repos || null,
      cron_expression || null,
      run_at || null,
      skills || [],
      mcp_servers || [],
      max_open_tasks ?? 3,
      task_prefix || null,
      priority || 'medium',
      cli || null,
      model || null,
    ]
  );

  return NextResponse.json(rows[0], { status: 201 });
}
