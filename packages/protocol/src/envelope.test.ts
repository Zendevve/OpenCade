import { describe, it, expect } from "vitest";
import { createEnvelope, validateEnvelope, parseEnvelope, serializeEnvelope } from "./envelope.js";
import { PROTOCOL_VERSION, isSupportedVersion } from "./version.js";
import type { PresencePayload, ChatPayload, ChallengePayload, SessionPayload, RoomPayload } from "./messages.js";

describe("version", () => {
  it("canonical is 1.0 and compat 1 accepted", () => {
    expect(PROTOCOL_VERSION).toBe("1.0");
    expect(isSupportedVersion("1.0")).toBe(true);
    expect(isSupportedVersion("1")).toBe(true);
    expect(isSupportedVersion("2.0")).toBe(false);
    expect(isSupportedVersion("")).toBe(false);
  });
});

describe("envelope roundtrip", () => {
  it("serializes type as type not msg_type", () => {
    const env = createEnvelope("health.ok", { status: "ok" });
    const raw = serializeEnvelope(env);
    const parsed = JSON.parse(raw);
    expect(parsed.type).toBe("health.ok");
    expect(parsed.msg_type).toBeUndefined();
    expect(parsed.version).toBe("1.0");
  });

  it("roundtrips presence", () => {
    const payload: PresencePayload = { user_id: "user-1", rtt_ms: 42, loss: 0.02, jitter_ms: 5, relay_reachable: true };
    const env = createEnvelope("presence.update", payload);
    const raw = serializeEnvelope(env);
    const back = parseEnvelope<PresencePayload>(raw);
    expect(back.payload).toEqual(payload);
    expect(back.version).toBe("1.0");
    expect(validateEnvelope(back)).toEqual({ ok: true });
  });

  it("roundtrips chat snake_case keys", () => {
    const payload: ChatPayload = { channel: "lobby:1", body: "hello", author_id: "user-1" };
    const env = createEnvelope("chat.message", payload);
    const raw = serializeEnvelope(env);
    const parsed = JSON.parse(raw);
    expect(parsed.payload.author_id).toBe("user-1");
    expect(parsed.payload.authorId).toBeUndefined();
    const back = parseEnvelope<ChatPayload>(raw);
    expect(back.payload).toEqual(payload);
  });

  it("roundtrips challenge and session and room", () => {
    const challenge: ChallengePayload = { game_id: "kof98", challenger_id: "user-1", challenged_id: "user-2" };
    const cEnv = createEnvelope("challenge.create", challenge);
    expect(parseEnvelope<ChallengePayload>(serializeEnvelope(cEnv)).payload).toEqual(challenge);

    const session: SessionPayload = { room_id: "room-1", sdp: "v=0\r\n...", candidate: "candidate:1" };
    const sEnv = createEnvelope("signaling.offer", session);
    expect(parseEnvelope<SessionPayload>(serializeEnvelope(sEnv)).payload).toEqual(session);

    const room: RoomPayload = { id: "room-1", game_id: "kof98", host_id: "user-1", guest_id: "user-2", state: "waiting" };
    const rEnv = createEnvelope("room.state", room);
    expect(parseEnvelope<RoomPayload>(serializeEnvelope(rEnv)).payload).toEqual(room);
  });

  it("validates version and type", () => {
    const ok = createEnvelope("test.event", {});
    expect(validateEnvelope(ok).ok).toBe(true);
    const badVersion = createEnvelope("test.event", {}, { version: "2.0" });
    expect(validateEnvelope(badVersion).ok).toBe(false);
    const compat = createEnvelope("test.event", {}, { version: "1" });
    expect(validateEnvelope(compat).ok).toBe(true);
    const emptyType = createEnvelope("", {}, {});
    expect(validateEnvelope(emptyType).ok).toBe(false);
  });

  it("timestamp is ISO-8601", () => {
    const env = createEnvelope("ping", {});
    expect(Number.isNaN(Date.parse(env.timestamp))).toBe(false);
    expect(validateEnvelope(env).ok).toBe(true);
  });

  it("compat with Rust envelope: camel vs snake and type field", () => {
    const rustJson = JSON.stringify({
      type: "presence.update",
      version: "1.0",
      request_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      timestamp: new Date().toISOString(),
      payload: { user_id: "user-1", rtt_ms: 42, loss: 0.5, jitter_ms: 7, relay_reachable: true },
    });
    const parsed = parseEnvelope<PresencePayload>(rustJson);
    expect(parsed.type).toBe("presence.update");
    expect(parsed.version).toBe("1.0");
    expect(parsed.payload.rtt_ms).toBe(42);
    expect(validateEnvelope(parsed).ok).toBe(true);
  });

  it("rejects invalid shape", () => {
    expect(() => parseEnvelope('{"type":"x"}')).toThrow();
  });
});
