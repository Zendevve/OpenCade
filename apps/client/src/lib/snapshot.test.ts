import { describe, expect, it } from "vitest";
import type { RoomSnapshotPayload } from "@opencade/protocol";
import { isRoomSnapshot, newestSnapshot } from "./snapshot";

const snapshot = (revision: number): RoomSnapshotPayload => ({
  room: {
    id: "room-1",
    game_id: "sfiii3",
    host_id: "host",
    guest_id: "guest",
    state: "connecting",
  },
  revision,
  preflight_count: 2,
  compatibility_matched: true,
  controller_ready_count: 2,
  barrier: {
    room_id: "room-1",
    ready_count: 2,
    required_count: 2,
    launch_at: "2026-08-25T12:00:00Z",
  },
  route: "direct_lan",
  route_policy: {
    route: "direct_lan",
    reason: "tcp_tunnel_operator_disabled",
    evidence_attempts: 0,
    evidence_verified: 0,
  },
});

describe("newestSnapshot", () => {
  it("rejects stale reconnect events and accepts idempotent duplicates", () => {
    expect(newestSnapshot(snapshot(8), snapshot(7)).revision).toBe(8);
    expect(newestSnapshot(snapshot(8), snapshot(8)).revision).toBe(8);
    expect(newestSnapshot(snapshot(8), snapshot(9)).revision).toBe(9);
  });

  it("validates every launch-critical snapshot field", () => {
    expect(isRoomSnapshot(snapshot(8))).toBe(true);
    expect(isRoomSnapshot({ ...snapshot(8), controller_ready_count: undefined })).toBe(false);
    expect(isRoomSnapshot({ ...snapshot(8), route_policy: undefined })).toBe(false);
    expect(
      isRoomSnapshot({
        ...snapshot(8),
        route_policy: { ...snapshot(8).route_policy, route: "tcp_tunnel" },
      })
    ).toBe(false);
    expect(
      isRoomSnapshot({
        ...snapshot(8),
        barrier: { ...snapshot(8).barrier, room_id: "another-room" },
      })
    ).toBe(false);
  });
});
