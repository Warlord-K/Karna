import NextAuth, { type NextAuthConfig } from "next-auth";
import Credentials from "next-auth/providers/credentials";
import Google from "next-auth/providers/google";
import PostgresAdapter from "@auth/pg-adapter";
import { compare } from "bcryptjs";
import { getPool } from "@/lib/db";
import authConfig from "./auth.config";

export { authDisabled } from "./auth.config";

export const googleEnabled = !!(
  process.env.AUTH_GOOGLE_ID && process.env.AUTH_GOOGLE_SECRET
);

const allowedEmailDomains = (process.env.AUTH_ALLOWED_EMAIL_DOMAINS || "")
  .split(",")
  .map((d) => d.trim().toLowerCase())
  .filter(Boolean);

const providers: NextAuthConfig["providers"] = [
  Credentials({
    credentials: {
      email: { label: "Email", type: "email" },
      password: { label: "Password", type: "password" },
    },
    async authorize(credentials) {
      const email = credentials?.email as string | undefined;
      const password = credentials?.password as string | undefined;
      if (!email || !password) return null;

      const pool = getPool();
      const { rows } = await pool.query(
        `SELECT id, name, email, password FROM users WHERE email = $1`,
        [email]
      );
      const user = rows[0];
      if (!user?.password) return null;

      const valid = await compare(password, user.password);
      if (!valid) return null;

      return { id: user.id, name: user.name, email: user.email };
    },
  }),
];

if (googleEnabled) {
  providers.push(
    Google({
      clientId: process.env.AUTH_GOOGLE_ID!,
      clientSecret: process.env.AUTH_GOOGLE_SECRET!,
      // Allow Google sign-in to link to an existing users row with the same email
      // (e.g. one seeded via credentials signup). Safe because Google verifies email.
      allowDangerousEmailAccountLinking: true,
    })
  );
}

export const { handlers, auth, signIn, signOut } = NextAuth({
  ...authConfig,
  adapter: PostgresAdapter(getPool()),
  providers,
  callbacks: {
    signIn({ user, account }) {
      if (account?.provider !== "google") return true;
      if (allowedEmailDomains.length === 0) return true;
      const email = (user.email || "").toLowerCase();
      const domain = email.split("@")[1];
      return !!domain && allowedEmailDomains.includes(domain);
    },
    jwt({ token, user }) {
      if (user) {
        token.id = user.id;
      }
      return token;
    },
    session({ session, token }) {
      if (session.user && token.id) {
        session.user.id = token.id as string;
      }
      return session;
    },
  },
});
