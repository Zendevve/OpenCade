import { describe, expect, it } from "vitest";
import type { RoomSnapshotPayload } from "@opencade/protocol";
import { newestSnapshot } from "./snapshot";

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
  barrier: {
    room_id: "room-1",
    ready_count: 2,
    required_count: 2,
    launch_at: "2026-08-25T12:00:00Z",
  },
  route: "direct_lan",
});

describe("newestSnapshot", () => {
  it("rejects stale reconnect events and accepts idempotent duplicates", () => {
    expect(newestSnapshot(snapshot(8), snapshot(7)).revision).toBe(8);
    expect(newestSnapshot(snapshot(8), snapshot(8)).revision).toBe(8);
    expect(newestSnapshot(snapshot(8), snapshot(9)).revision).toBe(9);
  });
});
