import NextAuth from "next-auth";
import authConfig, { authDisabled } from "./auth.config";

const { auth } = NextAuth(authConfig);

export default auth((req) => {
  if (authDisabled) return;

  const isLoggedIn = !!req.auth;
  const isLoginPage = req.nextUrl.pathname === "/login";
  const isAuthRoute = req.nextUrl.pathname.startsWith("/api/auth");

  if (isAuthRoute) return;

  if (!isLoggedIn && !isLoginPage) {
    return Response.redirect(new URL("/login", req.nextUrl));
  }

  if (isLoggedIn && isLoginPage) {
    return Response.redirect(new URL("/", req.nextUrl));
  }
});

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|.*\\.png$|.*\\.ico$|.*\\.svg$|.*\\.jpg$|.*\\.jpeg$|.*\\.webp$).*)"],
};
