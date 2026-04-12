import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function DELETE(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { id } = await params;

  const { rowCount } = await pool.query(
    "DELETE FROM repo_profiles WHERE id = $1 AND (user_id = $2 OR user_id = '00000000-0000-0000-0000-000000000000')",
    [id, userId]
  );

  if (rowCount === 0) {
    return NextResponse.json({ error: "Not found" }, { status: 404 });
  }

  return NextResponse.json({ ok: true });
}
