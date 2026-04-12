import LoginForm from "./login-form";

export const dynamic = "force-dynamic";

export default function LoginPage() {
  const signupDisabled = process.env.SIGNUP_DISABLED !== "false";
  return <LoginForm signupDisabled={signupDisabled} />;
}
