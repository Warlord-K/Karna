import { NextResponse } from "next/server";
import { hash } from "bcryptjs";
import { getPool } from "@/lib/db";

export async function POST(req: Request) {
  if (process.env.SIGNUP_DISABLED !== "false") {
    return NextResponse.json({ error: "Signups are disabled" }, { status: 403 });
  }

  const { email, password, name } = await req.json();

  if (!email || !password) {
    return NextResponse.json({ error: "Email and password required" }, { status: 400 });
  }

  if (password.length < 6) {
    return NextResponse.json({ error: "Password must be at least 6 characters" }, { status: 400 });
  }

  const pool = getPool();

  const existing = await pool.query(`SELECT id FROM users WHERE email = $1`, [email]);
  if (existing.rows.length > 0) {
    return NextResponse.json({ error: "Email already registered" }, { status: 409 });
  }

  const hashed = await hash(password, 12);
  await pool.query(
    `INSERT INTO users (name, email, password) VALUES ($1, $2, $3)`,
    [name || null, email, hashed]
  );

  return NextResponse.json({ ok: true });
}
