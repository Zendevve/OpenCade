import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { Envelope } from "@opencade/protocol";
import DiagnosticsButton from "./components/DiagnosticsButton";
import { ApiError, api } from "./lib/api";
import { useSessionStore } from "./lib/store";
import { OpenCadeSocket, type ConnectionState } from "./lib/ws";
import Auth from "./routes/Auth";
import Games from "./routes/Games";
import Lobby from "./routes/Lobby";
import Match from "./routes/Match";

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8080";
type View =
  { name: "games" } | { name: "lobby"; gameId: string } | { name: "match"; roomId: string };

export default function App() {
  const { token, user, setSession, clearSession } = useSessionStore();
  const queryClient = useQueryClient();
  const [view, setView] = useState<View>({ name: "games" });
  const [connection, setConnection] = useState<ConnectionState>("idle");
  const me = useQuery({
    queryKey: ["me", token],
    queryFn: () => {
      if (!token) throw new Error("session token unavailable");
      return api.me(token);
    },
    enabled: Boolean(token && !user),
    retry: false,
  });

  useEffect(() => {
    if (token && me.data?.user) setSession(token, me.data.user);
    if (me.error instanceof ApiError && me.error.status === 401) clearSession();
  }, [token, me.data, me.error, setSession, clearSession]);

  useEffect(() => {
    if (!token) return;
    const socket = new OpenCadeSocket(API_URL, token, setConnection);
    const unsubscribe = socket.subscribe((message: Envelope<unknown>) => {
      if (message.type.startsWith("challenge.")) {
        void queryClient.invalidateQueries({ queryKey: ["challenges"] });
      }
      if (message.type === "challenge.accepted") {
        const roomId = roomIdFromPayload(message.payload);
        if (roomId) setView({ name: "match", roomId });
      }
    });
    socket.connect();
    return () => {
      unsubscribe();
      socket.close();
    };
  }, [token, queryClient]);

  if (!token) return <Auth onAuthenticated={setSession} />;
  if (!user) {
    return (
      <main className="center-stage">
        <div className="status-card">Restoring session…</div>
      </main>
    );
  }
  const logout = async () => {
    try {
      await api.logout(token);
    } finally {
      clearSession();
    }
  };
  return (
    <div className="app-shell">
      <header className="topbar">
        <button
          className="brand"
          onClick={() => setView({ name: "games" })}
          aria-label="OpenCade games"
        >
          <span className="brand-glyph">OF</span>
          <span>OpenCade</span>
        </button>
        <div className="session-meta">
          <span className={`connection ${connection}`}>{connection}</span>
          <DiagnosticsButton />
          <span className="username">{user.username}</span>
          <button className="text-button" onClick={() => void logout()}>
            Sign out
          </button>
        </div>
      </header>
      <main>
        {view.name === "games" && (
          <Games token={token} onSelect={(gameId) => setView({ name: "lobby", gameId })} />
        )}
        {view.name === "lobby" && (
          <Lobby
            token={token}
            userId={user.id}
            gameId={view.gameId}
            onBack={() => setView({ name: "games" })}
            onMatch={(roomId) => setView({ name: "match", roomId })}
          />
        )}
        {view.name === "match" && (
          <Match token={token} roomId={view.roomId} onDone={() => setView({ name: "games" })} />
        )}
      </main>
    </div>
  );
}

function roomIdFromPayload(payload: unknown): string | undefined {
  if (typeof payload !== "object" || payload === null || !("room_id" in payload)) return undefined;
  const roomId = Reflect.get(payload, "room_id");
  return typeof roomId === "string" ? roomId : undefined;
}
