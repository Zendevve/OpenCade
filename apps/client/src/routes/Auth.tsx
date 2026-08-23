import { FormEvent, useState } from "react";
import { api, type User } from "../lib/api";

export default function Auth({
  onAuthenticated,
}: {
  onAuthenticated: (token: string, user: User) => void;
}) {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [identifier, setIdentifier] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const result =
        mode === "login"
          ? await api.login(identifier, password)
          : await api.register(identifier, email, password);
      onAuthenticated(result.token, result.user);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Authentication failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="auth-layout">
      <section className="auth-intro">
        <span className="brand-glyph large">OF</span>
        <p className="eyebrow">Open-source arcade netplay</p>
        <h1>
          Find the fight.
          <br />
          Own the stack.
        </h1>
        <p>One auditable path from lobby to peer connection and safe emulator launch.</p>
      </section>
      <form className="auth-card" onSubmit={(event) => void submit(event)}>
        <div className="segmented" aria-label="Authentication mode">
          <button
            type="button"
            className={mode === "login" ? "active" : ""}
            onClick={() => setMode("login")}
          >
            Sign in
          </button>
          <button
            type="button"
            className={mode === "register" ? "active" : ""}
            onClick={() => setMode("register")}
          >
            Create account
          </button>
        </div>
        <label>
          {mode === "login" ? "Username or email" : "Username"}
          <input
            required
            minLength={3}
            autoComplete="username"
            value={identifier}
            onChange={(event) => setIdentifier(event.target.value)}
          />
        </label>
        {mode === "register" && (
          <label>
            Email
            <input
              required
              type="email"
              autoComplete="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </label>
        )}
        <label>
          Password
          <input
            required
            minLength={8}
            type="password"
            autoComplete={mode === "login" ? "current-password" : "new-password"}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <button className="primary" disabled={busy}>
          {busy ? "Working…" : mode === "login" ? "Enter OpenFight" : "Create account"}
        </button>
      </form>
    </main>
  );
}
