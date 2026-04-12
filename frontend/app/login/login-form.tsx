'use client';

import { signIn } from "next-auth/react";
import { useState } from "react";
import { ArrowRight } from "@phosphor-icons/react/dist/ssr";

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
    "w-full h-9 px-3 text-[13px] rounded-lg bg-gray-2 border border-gray-4 text-gray-12 placeholder:text-gray-8 focus:outline-none focus:border-sun-7 focus:ring-1 focus:ring-sun-5 transition-colors";

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <div className="w-full max-w-[340px] mx-4">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-11 h-11 rounded-xl bg-sun-3 mb-4 shadow-card">
            <img src="/logo-192.png" alt="Karna" width={24} height={24} />
          </div>
          <h1 className="text-[15px] font-semibold text-gray-12 tracking-[-0.01em]">Sign in to Karna</h1>
          <p className="text-[13px] text-gray-9 mt-1">Autonomous coding agent</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-3">
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
            className="w-full h-9 rounded-lg bg-sun-9 text-gray-1 text-[13px] font-semibold hover:bg-sun-10 transition-colors disabled:opacity-50 flex items-center justify-center gap-1.5 mt-1"
          >
            {loading ? (
              <div className="w-3.5 h-3.5 border-[1.5px] border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                {isSignUp ? "Create account" : "Continue"}
                <ArrowRight size={14} weight="bold" />
              </>
            )}
          </button>
        </form>

        {!signupDisabled && (
          <div className="mt-6 pt-4 border-t border-gray-3 text-center">
            <button
              onClick={() => { setIsSignUp(!isSignUp); setError(""); }}
              className="text-[12px] text-gray-9 hover:text-gray-11 transition-colors"
            >
              {isSignUp ? "Already have an account? Sign in" : "Don't have an account? Sign up"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
