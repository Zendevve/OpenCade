export type MatchPhase =
  | "probing"
  | "awaiting_peer"
  | "ready"
  | "relay_probe_only"
  | "launching"
  | "awaiting_peer_launch"
  | "playing"
  | "finished"
  | "failed";

export type MatchCoordinatorState = {
  phase: MatchPhase;
  transport?: "direct_udp" | "relay";
  error?: string;
};

export type MatchCoordinatorEvent =
  | {
      type: "probe_verified";
      transport: "direct_udp" | "relay";
      candidate: "host" | "reflexive";
    }
  | { type: "peer_transcript_verified" }
  | { type: "launch_requested" }
  | { type: "native_spawned" }
  | { type: "room_playing" }
  | { type: "native_exited" }
  | { type: "room_finished" }
  | { type: "failed"; error: string }
  | { type: "reset" };

export const initialMatchCoordinatorState: MatchCoordinatorState = { phase: "probing" };

export function transitionMatchCoordinator(
  state: MatchCoordinatorState,
  event: MatchCoordinatorEvent
): MatchCoordinatorState {
  if (event.type === "reset") return initialMatchCoordinatorState;
  if (event.type === "failed") return { ...state, phase: "failed", error: event.error };
  if (event.type === "room_finished") return { ...state, phase: "finished" };

  switch (state.phase) {
    case "probing":
      if (event.type === "probe_verified") {
        return {
          phase:
            event.transport === "direct_udp" && event.candidate === "host"
              ? "awaiting_peer"
              : "relay_probe_only",
          transport: event.transport,
        };
      }
      return state;
    case "awaiting_peer":
      return event.type === "peer_transcript_verified" ? { ...state, phase: "ready" } : state;
    case "ready":
      return event.type === "launch_requested" ? { ...state, phase: "launching" } : state;
    case "launching":
      return event.type === "native_spawned" ? { ...state, phase: "awaiting_peer_launch" } : state;
    case "awaiting_peer_launch":
      if (event.type === "room_playing") return { ...state, phase: "playing" };
      if (event.type === "native_exited") return { ...state, phase: "finished" };
      return state;
    case "playing":
      return event.type === "native_exited" ? { ...state, phase: "finished" } : state;
    case "relay_probe_only":
    case "finished":
    case "failed":
      return state;
  }
}
