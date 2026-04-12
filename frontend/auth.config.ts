import type { NextAuthConfig } from "next-auth";

export const authDisabled = process.env.AUTH_DISABLED === "true";

export default {
  session: { strategy: "jwt" },
  providers: [], // Credentials provider added in auth.ts (needs Node.js runtime for DB)
  pages: {
    signIn: "/login",
  },
} satisfies NextAuthConfig;
