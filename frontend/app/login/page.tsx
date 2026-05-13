import LoginForm from "./login-form";

export const dynamic = "force-dynamic";

export default function LoginPage() {
  const signupDisabled = process.env.SIGNUP_DISABLED !== "false";
  const googleEnabled = !!(process.env.AUTH_GOOGLE_ID && process.env.AUTH_GOOGLE_SECRET);
  return <LoginForm signupDisabled={signupDisabled} googleEnabled={googleEnabled} />;
}
