export type PresenceStatus = "online" | "away" | "in-game";

export type PresencePayload = {
  user_id: string;
  status?: PresenceStatus;
  game_id?: string | null;
  rtt_ms?: number;
  loss?: number;
  jitter_ms?: number;
  relay_reachable?: boolean;
};

export type ChatPayload = {
  channel: string;
  body: string;
  author_id: string;
  room_id?: string | null;
  lobby_id?: string | null;
};

export type ChallengePayload = {
  room_id?: string | null;
  game_id: string;
  challenger_id: string;
  challenged_id: string;
  challenge_id?: string | null;
};

export type SessionPayload = {
  room_id: string;
  sdp_type?: string | null;
  sdp?: string | null;
  candidate?: string | null;
  sdp_mid?: string | null;
  sdp_mline_index?: number | null;
};

export type RoomState = "waiting" | "challenging" | "connecting" | "playing" | "finished" | "cancelled" | "WAITING" | "READY" | "PLAYING" | "FINISHED" | "CANCELLED";

export type RoomPayload = {
  id: string;
  game_id: string;
  host_id: string;
  guest_id?: string | null;
  members?: string[];
  state: RoomState;
  max_players?: number;
};

export type HelloPayload = {
  message: string;
  protocol_version: string;
};

export type ErrorPayload = {
  code: string;
  message: string;
  request_id?: string | null;
};

export type EnvelopeType =
  | "presence.update"
  | "chat.message"
  | "challenge.create"
  | "challenge.accept"
  | "challenge.decline"
  | "challenge.cancel"
  | "signaling.offer"
  | "signaling.answer"
  | "signaling.candidate"
  | "room.state"
  | "connection.hello"
  | "health.ok"
  | "error"
  | "ping"
  | "pong";

export function isKnownEnvelopeType(type: string): type is EnvelopeType {
  const known: EnvelopeType[] = [
    "presence.update",
    "chat.message",
    "challenge.create",
    "challenge.accept",
    "challenge.decline",
    "challenge.cancel",
    "signaling.offer",
    "signaling.answer",
    "signaling.candidate",
    "room.state",
    "connection.hello",
    "health.ok",
    "error",
    "ping",
    "pong",
  ];
  return (known as string[]).includes(type);
}
