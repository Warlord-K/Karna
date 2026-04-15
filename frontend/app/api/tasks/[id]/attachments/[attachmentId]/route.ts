import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

const DEFAULT_USER_ID = "00000000-0000-0000-0000-000000000000";

/** DELETE /api/tasks/[id]/attachments/[attachmentId] — Delete a specific attachment */
export async function DELETE(
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

  const { rowCount } = await pool.query(
    "DELETE FROM task_attachments WHERE id = $1 AND task_id = $2",
    [attachmentId, id]
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Attachment not found" }, { status: 404 });
  }

  return NextResponse.json({ ok: true });
}
