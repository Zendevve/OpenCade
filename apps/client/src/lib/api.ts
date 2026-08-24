import type { Envelope, RoomPayload } from "@opencade/protocol";

export type User = { id: string; username: string; email?: string | null };
export type AuthPayload = { user: User; token: string; expires_at: string };
export type Game = {
  id: string;
  name: string;
  emulator: string;
  default_version: string | null;
};
export type LobbyMember = {
  user_id: string;
  username: string;
  rtt_ms: number | null;
};
export type Challenge = {
  id: string;
  room_id: string;
  game_id: string;
  challenger_id: string;
  challenged_id: string;
  state: "pending" | "accepted" | "declined" | "cancelled";
};
export type RelayTicket = {
  relay_url: string;
  ticket: {
    room_id: string;
    user_id: string;
    expires_at: number;
    signature: string;
  };
};
export type MatchLaunchGrant = { grant: string; expires_at: string };

type ErrorPayload = { code?: string; message?: string };

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string
  ) {
    super(message);
    this.name = "ApiError";
  }
}

let apiBase = import.meta.env.VITE_API_URL?.replace(/\/$/, "") ?? "http://localhost:8080";

export function configureApiBase(url: string): void {
  const parsed = new URL(url);
  if (!(["http:", "https:"] as string[]).includes(parsed.protocol)) {
    throw new Error("OpenCade API URL must use HTTP or HTTPS");
  }
  if (
    parsed.protocol === "http:" &&
    !["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname)
  ) {
    throw new Error("Remote OpenCade API URLs must use HTTPS");
  }
  if (parsed.username || parsed.password || parsed.search || parsed.hash) {
    throw new Error("OpenCade API URL must not contain credentials, a query, or a fragment");
  }
  apiBase = parsed.toString().replace(/\/$/, "");
}

export function getApiBase(): string {
  return apiBase;
}

async function request<T>(path: string, token?: string | null, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  headers.set("Accept", "application/json");
  if (init?.body) headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let response: Response;
  try {
    response = await fetch(`${apiBase}${path}`, { ...init, headers });
  } catch {
    throw new ApiError(0, "network_unavailable", "OpenCade server is unreachable");
  }
  const decoded: unknown = await response.json().catch(() => null);
  const envelope = isEnvelope(decoded) ? decoded : null;
  if (!response.ok) {
    const payload = errorPayload(envelope?.payload);
    throw new ApiError(
      response.status,
      payload.code ?? "request_failed",
      payload.message ?? `Request failed (${response.status})`
    );
  }
  if (!envelope || envelope.payload === undefined) {
    throw new ApiError(response.status, "invalid_response", "Server returned an invalid envelope");
  }
  return envelope.payload as T;
}

function isEnvelope(value: unknown): value is Envelope<unknown> {
  return typeof value === "object" && value !== null && "payload" in value && "type" in value;
}

function errorPayload(value: unknown): ErrorPayload {
  if (typeof value !== "object" || value === null) return {};
  const code = Reflect.get(value, "code");
  const message = Reflect.get(value, "message");
  return {
    code: typeof code === "string" ? code : undefined,
    message: typeof message === "string" ? message : undefined,
  };
}

const post = (body?: unknown): RequestInit => ({
  method: "POST",
  body: body === undefined ? undefined : JSON.stringify(body),
});

export const api = {
  register: (username: string, email: string, password: string) =>
    request<AuthPayload>("/api/v1/auth/register", null, post({ username, email, password })),
  login: (identifier: string, password: string) =>
    request<AuthPayload>("/api/v1/auth/login", null, post({ identifier, password })),
  me: (token: string) => request<{ user: User }>("/api/v1/auth/me", token),
  logout: (token: string) => request<unknown>("/api/v1/auth/logout", token, post()),
  games: (token: string) => request<{ games: Game[] }>("/api/v1/games", token),
  lobby: (token: string, gameId: string) =>
    request<{ game_id: string; members: LobbyMember[] }>(
      `/api/v1/lobbies/${encodeURIComponent(gameId)}`,
      token
    ),
  joinLobby: (token: string, gameId: string) =>
    request<RoomPayload>("/api/v1/rooms", token, post({ game_id: gameId })),
  incomingChallenges: (token: string) =>
    request<{ challenges: Challenge[] }>("/api/v1/challenges", token),
  challenge: (token: string, gameId: string, challengedId: string) =>
    request<Challenge>(
      "/api/v1/challenges",
      token,
      post({ game_id: gameId, challenged_id: challengedId })
    ),
  acceptChallenge: (token: string, challengeId: string) =>
    request<Challenge>(`/api/v1/challenges/${challengeId}/accept`, token, post()),
  declineChallenge: (token: string, challengeId: string) =>
    request<Challenge>(`/api/v1/challenges/${challengeId}/decline`, token, post()),
  room: (token: string, roomId: string) => request<RoomPayload>(`/api/v1/rooms/${roomId}`, token),
  startRoom: (token: string, roomId: string) =>
    request<RoomPayload>(`/api/v1/rooms/${roomId}/start`, token, post()),
  finishRoom: (token: string, roomId: string, exitCode?: number | null) =>
    request<RoomPayload>(
      `/api/v1/rooms/${roomId}/finish`,
      token,
      post({ exit_code: exitCode ?? null })
    ),
  createLaunchGrant: (
    token: string,
    roomId: string,
    localEndpoint: string,
    peerEndpoint: string,
    inputDelayFrames = 2
  ) =>
    request<MatchLaunchGrant>(
      `/api/v1/rooms/${roomId}/launch-grant`,
      token,
      post({
        local_endpoint: localEndpoint,
        peer_endpoint: peerEndpoint,
        input_delay_frames: inputDelayFrames,
      })
    ),
  relayTicket: (token: string, roomId: string) =>
    request<RelayTicket>(`/api/v1/rooms/${roomId}/relay-ticket`, token, post()),
};
