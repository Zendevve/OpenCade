import type { ChallengePayload } from "./generated/ChallengePayload.js";
import type { ChatPayload } from "./generated/ChatPayload.js";
import type { MatchEndpointPayload } from "./generated/MatchEndpointPayload.js";
import type { MatchProbeCompletedPayload } from "./generated/MatchProbeCompletedPayload.js";
import type { PresencePayload } from "./generated/PresencePayload.js";
import type { RoomPayload } from "./generated/RoomPayload.js";
import type { SessionAnswerPayload } from "./generated/SessionAnswerPayload.js";
import type { SessionCandidatePayload } from "./generated/SessionCandidatePayload.js";
import type { SessionOfferPayload } from "./generated/SessionOfferPayload.js";

export type {
  ChallengePayload,
  ChatPayload,
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
  PresencePayload,
  RoomPayload,
  SessionAnswerPayload,
  SessionCandidatePayload,
  SessionOfferPayload,
};
export type { ChallengeState } from "./generated/ChallengeState.js";
export type { RoomState } from "./generated/RoomState.js";

export type HelloPayload = {
  user_id: string;
  protocol_version: string;
};

export type ErrorPayload = {
  code: string;
  message: string;
};

export type EnvelopeType =
  | "presence.update"
  | "chat.message"
  | "challenge.created"
  | "challenge.accepted"
  | "challenge.declined"
  | "challenge.cancelled"
  | "challenges.incoming"
  | "signaling.offer"
  | "signaling.answer"
  | "signaling.candidate"
  | "signaling.relayed"
  | "match.endpoint"
  | "match.endpoint.relayed"
  | "match.probe.completed"
  | "match.probe.completed.relayed"
  | "room.state"
  | "connection.hello"
  | "error"
  | "ping"
  | "pong";

const KNOWN_ENVELOPE_TYPES: ReadonlySet<string> = new Set<EnvelopeType>([
  "presence.update",
  "chat.message",
  "challenge.created",
  "challenge.accepted",
  "challenge.declined",
  "challenge.cancelled",
  "challenges.incoming",
  "signaling.offer",
  "signaling.answer",
  "signaling.candidate",
  "signaling.relayed",
  "match.endpoint",
  "match.endpoint.relayed",
  "match.probe.completed",
  "match.probe.completed.relayed",
  "room.state",
  "connection.hello",
  "error",
  "ping",
  "pong",
]);

export function isKnownEnvelopeType(type: string): type is EnvelopeType {
  return KNOWN_ENVELOPE_TYPES.has(type);
}
