import { invoke } from "@tauri-apps/api/tauri";

export type GameAvailability = { game_id: string; available: boolean; warnings: string[] };
export type NetworkDiagnostics = {
  nat: "unknown" | "cone" | "symmetric";
  rtt_ms: number | null;
  relay_reachable: boolean;
};
export type MatchEndpointCandidate = { endpoint: string; nonce: string };
export type MatchProbeReport = {
  room_id: string;
  local_user_id: string;
  peer_user_id: string;
  role: "host" | "guest";
  transport: "direct_udp";
  frames_sent: number;
  frames_received: number;
  transcript_checksum: string;
  elapsed_ms: number;
};

export function isDesktopRuntime(): boolean {
  return "__TAURI_IPC__" in window;
}

export async function scanGame(gameId: string): Promise<GameAvailability> {
  if (!isDesktopRuntime()) {
    return { game_id: gameId, available: false, warnings: ["Desktop scan unavailable"] };
  }
  return invoke<GameAvailability>("scan_game", { gameId });
}

export async function launchGame(gameId: string): Promise<number> {
  if (!isDesktopRuntime()) throw new Error("Emulator launch requires the desktop client");
  return invoke<number>("launch_game", { gameId });
}

export async function stopGame(pid: number): Promise<void> {
  if (!isDesktopRuntime()) return;
  await invoke("stop_game", { pid });
}

export async function runNetworkTest(): Promise<NetworkDiagnostics> {
  if (!isDesktopRuntime()) {
    return { nat: "unknown", rtt_ms: null, relay_reachable: false };
  }
  return invoke<NetworkDiagnostics>("network_test");
}

export async function reserveMatchProbe(roomId: string): Promise<MatchEndpointCandidate> {
  if (!isDesktopRuntime()) throw new Error("LAN match probe requires the desktop client");
  return invoke<MatchEndpointCandidate>("reserve_match_probe", {
    request: { room_id: roomId },
  });
}

export async function runReservedMatchProbe(request: {
  room_id: string;
  game_id: string;
  local_user_id: string;
  peer_user_id: string;
  role: "host" | "guest";
  peer_endpoint: string;
  peer_nonce: string;
  frame_count?: number;
  timeout_ms?: number;
}): Promise<MatchProbeReport> {
  if (!isDesktopRuntime()) throw new Error("LAN match probe requires the desktop client");
  return invoke<MatchProbeReport>("run_reserved_match_probe", { request });
}

export async function cancelMatchProbe(roomId: string): Promise<void> {
  if (!isDesktopRuntime()) return;
  await invoke("cancel_match_probe", { roomId });
}
