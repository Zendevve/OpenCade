import { describe, expect, it } from "vitest";
import type { RoomPayload } from "@opencade/protocol";
import {
  matchParticipants,
  nativeLanEndpoint,
  parseMatchCompletion,
  parseMatchEndpoint,
} from "./match";

const room = (guestId: string | null = "guest"): RoomPayload => ({
  id: "room-1",
  game_id: "sfiii3",
  host_id: "host",
  guest_id: guestId,
  state: "connecting",
});

describe("parseMatchEndpoint", () => {
  it("accepts the transport candidate fields", () => {
    expect(
      parseMatchEndpoint({
        room_id: "room-1",
        endpoint: "192.168.1.20:42000",
        reflexive_endpoint: "203.0.113.9:52000",
        nat: "mapped",
        nonce: "nonce-1",
      })
    ).toEqual({
      room_id: "room-1",
      endpoint: "192.168.1.20:42000",
      reflexive_endpoint: "203.0.113.9:52000",
      nat: "mapped",
      nonce: "nonce-1",
    });
  });

  it("defaults traversal evidence from an older version-1 peer", () => {
    expect(
      parseMatchEndpoint({
        room_id: "room-1",
        endpoint: "192.168.1.20:42000",
        nonce: "nonce-1",
      })
    ).toEqual({
      room_id: "room-1",
      endpoint: "192.168.1.20:42000",
      reflexive_endpoint: null,
      nat: "unknown",
      nonce: "nonce-1",
    });
  });

  it("rejects missing or non-string fields", () => {
    expect(parseMatchEndpoint(null)).toBeUndefined();
    expect(
      parseMatchEndpoint({ room_id: "room-1", endpoint: 42000, nonce: "nonce-1" })
    ).toBeUndefined();
    expect(parseMatchEndpoint({ room_id: "room-1", endpoint: "127.0.0.1:1" })).toBeUndefined();
    expect(
      parseMatchEndpoint({
        room_id: "room-1",
        endpoint: "127.0.0.1:1",
        reflexive_endpoint: null,
        nat: "symmetric",
        nonce: "nonce-1",
      })
    ).toBeUndefined();
  });
});

describe("matchParticipants", () => {
  it("derives complementary host and guest peers", () => {
    expect(matchParticipants(room(), "host")).toEqual({ role: "host", peerUserId: "guest" });
    expect(matchParticipants(room(), "guest")).toEqual({ role: "guest", peerUserId: "host" });
  });

  it("rejects incomplete rooms and non-members", () => {
    expect(matchParticipants(room(null), "host")).toBeUndefined();
    expect(matchParticipants(room(), "outsider")).toBeUndefined();
  });
});

describe("parseMatchCompletion", () => {
  it("accepts a peer transcript result", () => {
    expect(
      parseMatchCompletion({
        room_id: "room-1",
        frames_received: 60,
        transcript_checksum: "0376c2e852f4fd25",
      })
    ).toEqual({
      room_id: "room-1",
      frames_received: 60,
      transcript_checksum: "0376c2e852f4fd25",
    });
  });

  it("rejects incomplete peer results", () => {
    expect(parseMatchCompletion({ room_id: "room-1", frames_received: "60" })).toBeUndefined();
  });
});

describe("nativeLanEndpoint", () => {
  it("preserves the verified host and selects the RetroArch TCP port", () => {
    expect(nativeLanEndpoint("192.168.1.20:42000")).toBe("192.168.1.20:55435");
    expect(nativeLanEndpoint("[::1]:42000")).toBe("[::1]:55435");
  });
});
