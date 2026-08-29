import type { ControllerPreflightPayload, MatchReportRole } from "@opencade/protocol";

export function controllerPreflight(role: MatchReportRole): ControllerPreflightPayload {
  const pads = typeof navigator.getGamepads === "function" ? navigator.getGamepads() : [];
  return {
    connected: Array.from(pads).some((pad) => pad?.connected),
    player_slot: role === "host" ? 1 : 2,
  };
}
