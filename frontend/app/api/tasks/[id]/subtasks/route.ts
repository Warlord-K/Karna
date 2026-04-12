import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

/** GET /api/tasks/[id]/subtasks — List subtasks for a parent task */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  const { rows } = await pool.query(
    `SELECT * FROM agent_tasks
     WHERE parent_task_id = $1 AND user_id = $2
     ORDER BY position ASC, created_at ASC`,
    [id, userId]
  );

  return NextResponse.json(rows);
}

/** POST /api/tasks/[id]/subtasks — Parse subtasks from plan_content and create them.
 *  Called when user approves a parent task's plan. */
export async function POST(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  // Fetch parent task
  const { rows: taskRows } = await pool.query(
    "SELECT * FROM agent_tasks WHERE id = $1 AND user_id = $2",
    [id, userId]
  );

  if (taskRows.length === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const parentTask = taskRows[0];

  if (parentTask.status !== "plan_review") {
    return NextResponse.json(
      { error: "Task must be in plan_review status" },
      { status: 400 }
    );
  }

  if (!parentTask.plan_content) {
    return NextResponse.json(
      { error: "No plan content to parse subtasks from" },
      { status: 400 }
    );
  }

  // Parse subtasks from plan_content
  const match = parentTask.plan_content.match(
    /<!--\s*subtasks\s*\n([\s\S]*?)\nsubtasks\s*-->/
  );

  if (!match) {
    return NextResponse.json(
      { error: "No subtask definitions found in plan. Approve normally for single-repo tasks." },
      { status: 400 }
    );
  }

  let subtaskDefs: { title: string; repo: string; description?: string }[];
  try {
    subtaskDefs = JSON.parse(match[1]);
    if (!Array.isArray(subtaskDefs) || subtaskDefs.length === 0) {
      throw new Error("Empty subtask array");
    }
  } catch {
    return NextResponse.json(
      { error: "Failed to parse subtask definitions from plan" },
      { status: 400 }
    );
  }

  // Check for existing subtasks (idempotency)
  const { rows: existingSubs } = await pool.query(
    "SELECT id FROM agent_tasks WHERE parent_task_id = $1",
    [id]
  );

  if (existingSubs.length > 0) {
    return NextResponse.json(
      { error: "Subtasks already exist for this task" },
      { status: 409 }
    );
  }

  // Create subtasks
  const created: any[] = [];
  for (const def of subtaskDefs) {
    if (!def.title || !def.repo) continue;

    const { rows } = await pool.query(
      `INSERT INTO agent_tasks (user_id, parent_task_id, title, description, repo, priority, position, cli, model)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
       RETURNING *`,
      [
        userId,
        id,
        def.title,
        def.description || null,
        def.repo,
        parentTask.priority,
        Date.now() + created.length,
        parentTask.cli || null,
        parentTask.model || null,
      ]
    );
    created.push(rows[0]);
  }

  // Move parent to in_progress (waiting for subtasks)
  await pool.query(
    "UPDATE agent_tasks SET status = 'in_progress' WHERE id = $1",
    [id]
  );

  return NextResponse.json(created, { status: 201 });
}
