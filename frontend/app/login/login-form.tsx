'use client';

import { signIn } from "next-auth/react";
import { useState } from "react";
import { ArrowRight, GitPullRequest, Robot, Kanban, GithubLogo, Lightning } from "@phosphor-icons/react/dist/ssr";

const FEATURES = [
  {
    icon: Kanban,
    title: "Kanban Board",
    desc: "Create tasks, drag between columns, track progress visually.",
  },
  {
    icon: Robot,
    title: "AI Planning",
    desc: "Agent explores your codebase, generates a plan, and waits for approval.",
  },
  {
    icon: GitPullRequest,
    title: "Auto PRs",
    desc: "Implements the plan, opens a PR, and iterates on your feedback.",
  },
  {
    icon: Lightning,
    title: "Multi-Repo",
    desc: "Orchestrate changes across multiple repositories in a single task.",
  },
];

export default function LoginForm({ signupDisabled = false }: { signupDisabled?: boolean }) {
  const [isSignUp, setIsSignUp] = useState(false);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      if (isSignUp) {
        const res = await fetch("/api/auth/signup", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ email, password, name }),
        });
        const data = await res.json();
        if (!res.ok) {
          setError(data.error || "Signup failed");
          setLoading(false);
          return;
        }
      }

      const result = await signIn("credentials", {
        email,
        password,
        redirect: false,
      });

      if (result?.error) {
        setError("Invalid email or password");
        setLoading(false);
        return;
      }

      window.location.href = "/";
    } catch {
      setError("Something went wrong");
      setLoading(false);
    }
  }

  const inputClass =
    "w-full h-10 px-3 text-[14px] rounded-lg bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-8 focus:outline-none focus:border-sun-7 focus:ring-1 focus:ring-sun-5 transition-colors";

  return (
    <div className="min-h-screen bg-background flex flex-col">
      {/* Header */}
      <header className="flex items-center justify-between px-6 sm:px-10 h-14 flex-shrink-0">
        <div className="flex items-center gap-2.5">
          <img src="/logo-192.png" alt="Karna" width={22} height={22} />
          <span className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">Karna</span>
        </div>
        <a
          href="https://github.com/Warlord-K/karna"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-1.5 text-[13px] text-gray-8 hover:text-gray-11 transition-colors"
        >
          <GithubLogo size={16} weight="bold" />
          <span className="hidden sm:inline">GitHub</span>
        </a>
      </header>

      {/* Main content */}
      <main className="flex-1 flex flex-col lg:flex-row items-center justify-center gap-10 lg:gap-20 px-6 sm:px-10 py-10 lg:py-0">
        {/* Left — Hero */}
        <div className="max-w-lg text-center lg:text-left">
          <h1 className="text-[36px] sm:text-[44px] font-bold text-gray-12 tracking-[-0.03em] leading-[1.1]">
            Autonomous<br />
            <span className="text-sun-9">Coding Agent</span>
          </h1>
          <p className="text-[16px] sm:text-[17px] text-gray-9 mt-4 leading-relaxed max-w-md mx-auto lg:mx-0">
            Create tasks on a kanban board, an AI agent plans and implements them, opens PRs, and iterates on your feedback.
          </p>

          {/* Screenshot preview */}
          <div className="mt-8">
            <div className="rounded-xl overflow-hidden border border-gray-3/60 shadow-elevated">
              <img
                src="/Cover.png"
                alt="Karna dashboard preview"
                className="w-full h-auto"
              />
            </div>
          </div>

          {/* Feature grid */}
          <div className="grid grid-cols-2 gap-3 mt-6">
            {FEATURES.map((f) => (
              <div key={f.title} className="flex items-start gap-2.5 p-3 rounded-xl bg-gray-2/50 border border-gray-3/60">
                <f.icon size={18} weight="bold" className="text-sun-9 mt-0.5 flex-shrink-0" />
                <div>
                  <p className="text-[13px] font-medium text-gray-12">{f.title}</p>
                  <p className="text-[11px] text-gray-8 mt-0.5 leading-snug">{f.desc}</p>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Right — Auth form */}
        <div className="w-full max-w-[380px]">
          <div className="rounded-2xl bg-gray-2/40 border border-gray-3 p-6 sm:p-8 shadow-elevated">
            <div className="text-center mb-6">
              <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-sun-3 mb-4 shadow-card">
                <img src="/logo-192.png" alt="Karna" width={28} height={28} />
              </div>
              <h2 className="text-[17px] font-semibold text-gray-12 tracking-[-0.01em]">
                {isSignUp ? "Create your account" : "Welcome back"}
              </h2>
              <p className="text-[13px] text-gray-9 mt-1">
                {isSignUp ? "Get started with Karna" : "Sign in to your instance"}
              </p>
            </div>

            <form onSubmit={handleSubmit} className="space-y-3.5">
              {isSignUp && (
                <div>
                  <label className="block text-[12px] font-medium text-gray-9 mb-1.5">Name</label>
                  <input
                    type="text"
                    placeholder="Your name"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    className={inputClass}
                  />
                </div>
              )}
              <div>
                <label className="block text-[12px] font-medium text-gray-9 mb-1.5">Email</label>
                <input
                  type="email"
                  placeholder="you@company.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                  autoFocus
                  className={inputClass}
                />
              </div>
              <div>
                <label className="block text-[12px] font-medium text-gray-9 mb-1.5">Password</label>
                <input
                  type="password"
                  placeholder="Enter password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                  minLength={6}
                  className={inputClass}
                />
              </div>

              {error && (
                <p className="text-[12px] text-red-400 py-1">{error}</p>
              )}

              <button
                type="submit"
                disabled={loading}
                className="w-full h-10 rounded-lg bg-sun-9 text-gray-1 text-[14px] font-semibold hover:bg-sun-10 hover:shadow-[0_0_16px_hsl(40_90%_56%/0.25)] transition-all duration-200 disabled:opacity-50 flex items-center justify-center gap-1.5 mt-1"
              >
                {loading ? (
                  <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                ) : (
                  <>
                    {isSignUp ? "Create account" : "Continue"}
                    <ArrowRight size={15} weight="bold" />
                  </>
                )}
              </button>
            </form>

            {!signupDisabled && (
              <div className="mt-5 pt-4 border-t border-gray-3 text-center">
                <button
                  onClick={() => { setIsSignUp(!isSignUp); setError(""); }}
                  className="text-[13px] text-gray-9 hover:text-gray-11 transition-colors"
                >
                  {isSignUp ? "Already have an account? Sign in" : "Don't have an account? Sign up"}
                </button>
              </div>
            )}
          </div>

        </div>
      </main>

      {/* Decorative background */}
      <div className="board-bg-decoration" aria-hidden="true">
        <div className="board-diamond board-diamond--1" />
        <div className="board-diamond board-diamond--2" />
        <div className="board-diamond board-diamond--3" />
        <div className="board-diamond board-diamond--4" />
      </div>
    </div>
  );
}
