import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

const DEFAULT_USER_ID = "00000000-0000-0000-0000-000000000000";

/** GET /api/tasks/[id]/attachments/[attachmentId]/file — Serve image binary */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string; attachmentId: string }> }
) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id, attachmentId } = await params;

  // Verify task ownership
  const { rowCount: taskCount } = await pool.query(
    "SELECT 1 FROM agent_tasks WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
    [id, userId, DEFAULT_USER_ID]
  );

  if (taskCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  const { rows } = await pool.query(
    "SELECT data, content_type, filename FROM task_attachments WHERE id = $1 AND task_id = $2",
    [attachmentId, id]
  );

  if (rows.length === 0) {
    return NextResponse.json({ error: "Attachment not found" }, { status: 404 });
  }

  const { data, content_type, filename } = rows[0];

  return new NextResponse(data, {
    headers: {
      "Content-Type": content_type,
      "Content-Disposition": `inline; filename="${filename}"`,
      "Cache-Control": "private, max-age=3600",
    },
  });
}
