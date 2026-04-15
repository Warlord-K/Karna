import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

const DEFAULT_USER_ID = "00000000-0000-0000-0000-000000000000";
const ALLOWED_TYPES = ["image/jpeg", "image/png", "image/gif", "image/webp"];
const MAX_FILE_SIZE = 5 * 1024 * 1024; // 5MB
const MAX_ATTACHMENTS_PER_TASK = 10;

/** GET /api/tasks/[id]/attachments — List attachments (metadata only, no binary data) */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  const { rowCount } = await pool.query(
    "SELECT 1 FROM agent_tasks WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
    [id, userId, DEFAULT_USER_ID]
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const { rows } = await pool.query(
    "SELECT id, task_id, filename, content_type, size_bytes, created_at FROM task_attachments WHERE task_id = $1 ORDER BY created_at ASC",
    [id]
  );

  return NextResponse.json(rows);
}

/** POST /api/tasks/[id]/attachments — Upload an image attachment */
export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  const { rowCount } = await pool.query(
    "SELECT 1 FROM agent_tasks WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
    [id, userId, DEFAULT_USER_ID]
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  // Check attachment count limit
  const { rows: countRows } = await pool.query(
    "SELECT COUNT(*)::int as count FROM task_attachments WHERE task_id = $1",
    [id]
  );

  if (countRows[0].count >= MAX_ATTACHMENTS_PER_TASK) {
    return NextResponse.json(
      { error: `Maximum ${MAX_ATTACHMENTS_PER_TASK} attachments per task` },
      { status: 400 }
    );
  }

  const formData = await req.formData();
  const file = formData.get("file") as File | null;

  if (!file) {
    return NextResponse.json({ error: "No file provided" }, { status: 400 });
  }

  if (!ALLOWED_TYPES.includes(file.type)) {
    return NextResponse.json(
      { error: "Only JPEG, PNG, GIF, and WebP images are supported" },
      { status: 400 }
    );
  }

  if (file.size > MAX_FILE_SIZE) {
    return NextResponse.json(
      { error: "File size must be under 5MB" },
      { status: 400 }
    );
  }

  const buffer = Buffer.from(await file.arrayBuffer());

  const { rows } = await pool.query(
    `INSERT INTO task_attachments (task_id, filename, content_type, data, size_bytes)
     VALUES ($1, $2, $3, $4, $5)
     RETURNING id, task_id, filename, content_type, size_bytes, created_at`,
    [id, file.name || "image.png", file.type, buffer, file.size]
  );

  return NextResponse.json(rows[0], { status: 201 });
}
