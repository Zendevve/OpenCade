import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  Envelope,
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
} from "@opencade/protocol";
import DiagnosticsButton from "./components/DiagnosticsButton";
import { ApiError, api, configureApiBase } from "./lib/api";
import { parseMatchCompletion, parseMatchEndpoint } from "./lib/match";
import { cancelMatchProbe, loadRuntimeConfig } from "./lib/native";
import { useSessionStore } from "./lib/store";
import { OpenCadeSocket, type ConnectionState } from "./lib/ws";
import Auth from "./routes/Auth";
import Games from "./routes/Games";
import Lobby from "./routes/Lobby";
import Match from "./routes/Match";

type View =
  { name: "games" } | { name: "lobby"; gameId: string } | { name: "match"; roomId: string };

export default function App() {
  const { token, user, hydrated, hydrate, setSession, clearSession } = useSessionStore();
  const queryClient = useQueryClient();
  const [view, setView] = useState<View>({ name: "games" });
  const [connection, setConnection] = useState<ConnectionState>("idle");
  const [activeSocket, setActiveSocket] = useState<OpenCadeSocket | null>(null);
  const [peerEndpoint, setPeerEndpoint] = useState<MatchEndpointPayload | null>(null);
  const [peerCompletion, setPeerCompletion] = useState<MatchProbeCompletedPayload | null>(null);
  const [apiUrl, setApiUrl] = useState<string | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);

  useEffect(() => {
    void hydrate();
  }, [hydrate]);

  useEffect(() => {
    let cancelled = false;
    void loadRuntimeConfig()
      .then((config) => {
        configureApiBase(config.api_url);
        if (!cancelled) setApiUrl(config.api_url);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setRuntimeError(error instanceof Error ? error.message : "Runtime configuration failed");
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);
  const me = useQuery({
    queryKey: ["me", token],
    queryFn: () => {
      if (!token) throw new Error("session token unavailable");
      return api.me(token);
    },
    enabled: Boolean(apiUrl && token && !user),
    retry: false,
  });

  useEffect(() => {
    if (token && me.data?.user) setSession(token, me.data.user);
    if (me.error instanceof ApiError && me.error.status === 401) clearSession();
  }, [token, me.data, me.error, setSession, clearSession]);

  useEffect(() => {
    if (!apiUrl || !token) return;
    const socket = new OpenCadeSocket(apiUrl, token, setConnection);
    setActiveSocket(socket);
    const unsubscribe = socket.subscribe((message: Envelope<unknown>) => {
      if (message.type.startsWith("challenge.")) {
        void queryClient.invalidateQueries({ queryKey: ["challenges"] });
      }
      if (message.type === "challenge.accepted") {
        const roomId = roomIdFromPayload(message.payload);
        if (roomId) setView({ name: "match", roomId });
      }
      if (message.type === "match.endpoint") {
        const endpoint = parseMatchEndpoint(message.payload);
        if (endpoint) {
          setPeerEndpoint(endpoint);
        }
      }
      if (message.type === "match.probe.completed") {
        const completion = parseMatchCompletion(message.payload);
        if (completion) setPeerCompletion(completion);
      }
    });
    socket.connect();
    return () => {
      unsubscribe();
      socket.close();
      setActiveSocket((current) => (current === socket ? null : current));
    };
  }, [apiUrl, token, queryClient]);

  if (runtimeError) {
    return (
      <main className="center-stage">
        <div className="status-card">{runtimeError}</div>
      </main>
    );
  }
  if (!apiUrl || !hydrated) {
    return (
      <main className="center-stage">
        <div className="status-card">Restoring secure runtime state…</div>
      </main>
    );
  }

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
      if (view.name === "match") await cancelMatchProbe(view.roomId);
      await api.logout(token);
    } finally {
      clearSession();
    }
  };
  const returnToGames = () => {
    if (view.name === "match") void cancelMatchProbe(view.roomId);
    setPeerEndpoint(null);
    setPeerCompletion(null);
    setView({ name: "games" });
  };
  return (
    <div className="app-shell">
      <header className="topbar">
        <button className="brand" onClick={returnToGames} aria-label="OpenCade games">
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
          <Match
            token={token}
            userId={user.id}
            roomId={view.roomId}
            socket={activeSocket}
            peerEndpoint={peerEndpoint?.room_id === view.roomId ? peerEndpoint : undefined}
            peerCompletion={peerCompletion?.room_id === view.roomId ? peerCompletion : undefined}
            onProbeRetry={() => {
              setPeerEndpoint(null);
              setPeerCompletion(null);
            }}
            onDone={returnToGames}
          />
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
