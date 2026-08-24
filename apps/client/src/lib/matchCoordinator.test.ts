import { describe, expect, it } from "vitest";
import { initialMatchCoordinatorState, transitionMatchCoordinator } from "./matchCoordinator.js";

describe("match coordinator", () => {
  it("allows a direct verified match to become playable only after native spawn and peer launch", () => {
    let state = transitionMatchCoordinator(initialMatchCoordinatorState, {
      type: "probe_verified",
      transport: "direct_udp",
      candidate: "host",
    });
    expect(state.phase).toBe("awaiting_peer");
    state = transitionMatchCoordinator(state, { type: "peer_transcript_verified" });
    state = transitionMatchCoordinator(state, { type: "launch_requested" });
    state = transitionMatchCoordinator(state, { type: "native_spawned" });
    expect(state.phase).toBe("awaiting_peer_launch");
    expect(transitionMatchCoordinator(state, { type: "room_playing" }).phase).toBe("playing");
  });

  it("truthfully gates a relay probe from native launch", () => {
    const state = transitionMatchCoordinator(initialMatchCoordinatorState, {
      type: "probe_verified",
      transport: "relay",
      candidate: "host",
    });
    expect(state.phase).toBe("relay_probe_only");
    expect(transitionMatchCoordinator(state, { type: "launch_requested" })).toEqual(state);
  });

  it("does not claim a UDP reflexive candidate is a native TCP route", () => {
    const state = transitionMatchCoordinator(initialMatchCoordinatorState, {
      type: "probe_verified",
      transport: "direct_udp",
      candidate: "reflexive",
    });
    expect(state.phase).toBe("relay_probe_only");
  });
});
