export { PROTOCOL_VERSION, isSupportedVersion } from "./version.js";
export type { Envelope } from "./envelope.js";
export { createEnvelope, validateEnvelope, parseEnvelope, serializeEnvelope } from "./envelope.js";
export type {
  PresencePayload,
  ChatPayload,
  ChallengePayload,
  SessionPayload,
  RoomPayload,
  RoomState,
  HelloPayload,
  ErrorPayload,
  EnvelopeType,
  PresenceStatus,
} from "./messages.js";
export { isKnownEnvelopeType } from "./messages.js";
