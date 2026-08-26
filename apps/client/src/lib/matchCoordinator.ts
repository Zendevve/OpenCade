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

type Transport = "direct_udp" | "relay";

export type MatchCoordinatorState =
  | { phase: "probing"; transport?: never; error?: never }
  | {
      phase: "awaiting_peer" | "ready" | "launching" | "awaiting_peer_launch" | "playing";
      transport: "direct_udp";
      error?: never;
    }
  | { phase: "relay_probe_only"; transport: "relay"; error?: never }
  | { phase: "finished"; transport?: Transport; error?: never }
  | { phase: "failed"; transport?: Transport; error: string };

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
  if (event.type === "failed") {
    return { phase: "failed", transport: state.transport, error: event.error };
  }
  if (event.type === "room_finished") return { phase: "finished", transport: state.transport };

  switch (state.phase) {
    case "probing":
      if (event.type === "probe_verified") {
        return event.transport === "direct_udp" && event.candidate === "host"
          ? { phase: "awaiting_peer", transport: "direct_udp" }
          : { phase: "relay_probe_only", transport: "relay" };
      }
      return state;
    case "awaiting_peer":
      return event.type === "peer_transcript_verified"
        ? { phase: "ready", transport: state.transport }
        : state;
    case "ready":
      return event.type === "launch_requested"
        ? { phase: "launching", transport: state.transport }
        : state;
    case "launching":
      return event.type === "native_spawned"
        ? { phase: "awaiting_peer_launch", transport: state.transport }
        : state;
    case "awaiting_peer_launch":
      if (event.type === "room_playing") {
        return { phase: "playing", transport: state.transport };
      }
      if (event.type === "native_exited") {
        return { phase: "finished", transport: state.transport };
      }
      return state;
    case "playing":
      return event.type === "native_exited"
        ? { phase: "finished", transport: state.transport }
        : state;
    case "relay_probe_only":
    case "finished":
    case "failed":
      return state;
  }
}
