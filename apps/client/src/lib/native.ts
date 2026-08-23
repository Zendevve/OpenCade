import { invoke } from "@tauri-apps/api/tauri";

export type GameAvailability = { game_id: string; available: boolean; warnings: string[] };
export type NetworkDiagnostics = {
  nat: "unknown" | "cone" | "symmetric";
  rtt_ms: number | null;
  relay_reachable: boolean;
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
