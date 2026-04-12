import { NextRequest, NextResponse } from "next/server";
import { getRequiredUserId } from "@/lib/api-auth";
import { pool } from "@/lib/db";

export async function GET() {
  const userId = await getRequiredUserId();
  if (!userId) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const { rows } = await pool.query(
    `SELECT * FROM repo_profiles
     WHERE user_id = $1 OR user_id = '00000000-0000-0000-0000-000000000000'
     ORDER BY repo ASC`,
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
  const { repo, branch } = body;

  if (!repo?.trim()) {
    return NextResponse.json({ error: "repo is required (owner/repo format)" }, { status: 400 });
  }

  // Validate owner/repo format
  if (!repo.includes('/')) {
    return NextResponse.json({ error: "repo must be in owner/repo format" }, { status: 400 });
  }

  const { rows } = await pool.query(
    `INSERT INTO repo_profiles (user_id, repo, branch, status)
     VALUES ($1, $2, $3, 'pending')
     ON CONFLICT (repo) DO UPDATE SET
       branch = EXCLUDED.branch,
       status = CASE
         WHEN repo_profiles.status IN ('ready', 'stale', 'failed') THEN 'pending'
         ELSE repo_profiles.status
       END
     RETURNING *`,
    [userId, repo.trim(), branch?.trim() || 'main']
  );

  return NextResponse.json(rows[0], { status: 201 });
}
