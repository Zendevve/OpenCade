import type { ControllerPreflightPayload, MatchReportRole } from "@opencade/protocol";

/**
 * Threshold: release requires a physical pad (connected === true). In dev (Vite DEV)
 * the loopback proof-of-match path mocks connected=true so a fresh dev without a second
 * controller can still reach canContinue. Production stays fail-closed.
 */
export function controllerPreflight(role: MatchReportRole): ControllerPreflightPayload {
  const pads = typeof navigator.getGamepads === "function" ? navigator.getGamepads() : [];
  const physicalConnected = Array.from(pads).some((pad) => pad?.connected);
  if (physicalConnected) {
    return {
      connected: true,
      player_slot: role === "host" ? 1 : 2,
    };
  }
  let isDev = false;
  if (
    typeof import.meta !== "undefined" &&
    import.meta &&
    typeof import.meta === "object" &&
    "env" in import.meta
  ) {
    const env = import.meta.env;
    if (env && typeof env === "object" && "DEV" in env) {
      isDev = Boolean(env.DEV);
    }
  }
  if (isDev) {
    return {
      connected: true,
      player_slot: role === "host" ? 1 : 2,
    };
  }
  return {
    connected: false,
    player_slot: role === "host" ? 1 : 2,
  };
}
