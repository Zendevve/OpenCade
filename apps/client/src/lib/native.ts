import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type GameAvailability = { game_id: string; available: boolean; warnings: string[] };
export type NetworkDiagnostics = {
  nat: "unknown" | "open" | "mapped" | "blocked";
  rtt_ms: number | null;
  relay_reachable: boolean;
};
export type MatchEndpointCandidate = {
  endpoint: string;
  reflexive_endpoint?: string;
  nat: "unknown" | "open" | "mapped";
  nonce: string;
};
export type MatchProbeReport = {
  room_id: string;
  local_user_id: string;
  peer_user_id: string;
  role: "host" | "guest";
  transport: "direct_udp" | "relay";
  frames_sent: number;
  frames_received: number;
  transcript_checksum: string;
  elapsed_ms: number;
  nat: "unknown" | "open" | "mapped";
  candidate: "host" | "reflexive";
  punch_attempts: number;
};
export type RetroarchMatchLaunch = {
  pid: number;
  adapter: "retroarch_fbneo";
  room_id: string;
  fingerprint: {
    retroarch_version?: string | null;
    executable_sha256: string;
    core_sha256: string;
    content_sha256: string;
  };
};
export type EmulatorExitEvent = {
  pid: number;
  room_id?: string | null;
  exit_code?: number | null;
  success: boolean;
};
export type RuntimeConfig = { api_url: string; stun_server?: string | null };

let runtimeStunServer = import.meta.env.VITE_STUN_SERVER || undefined;

export function isDesktopRuntime(): boolean {
  return "__TAURI_IPC__" in window;
}

export async function loadRuntimeConfig(): Promise<RuntimeConfig> {
  if (!isDesktopRuntime()) {
    return {
      api_url: import.meta.env.VITE_API_URL ?? "http://localhost:8080",
      stun_server: runtimeStunServer,
    };
  }
  const config = await invoke<RuntimeConfig>("runtime_config");
  runtimeStunServer = config.stun_server ?? undefined;
  return config;
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

export async function launchRetroarchMatch(request: {
  api_url: string;
  session_token: string;
  launch_grant: string;
}): Promise<RetroarchMatchLaunch> {
  if (!isDesktopRuntime()) throw new Error("RetroArch netplay requires the desktop client");
  return invoke<RetroarchMatchLaunch>("launch_retroarch_match", { request });
}

export async function onEmulatorExit(
  handler: (event: EmulatorExitEvent) => void
): Promise<UnlistenFn> {
  if (!isDesktopRuntime()) return () => undefined;
  return listen<EmulatorExitEvent>("opencade://emulator-exited", (event) => handler(event.payload));
}

const WEB_SESSION_KEY = "opencade.session_token";

export async function storeSessionToken(token: string): Promise<void> {
  if (isDesktopRuntime()) {
    await invoke("store_session_token", { token });
    return;
  }
  sessionStorage.setItem(WEB_SESSION_KEY, token);
}

export async function loadSessionToken(): Promise<string | null> {
  localStorage.removeItem(WEB_SESSION_KEY);
  if (isDesktopRuntime()) return invoke<string | null>("load_session_token");
  return sessionStorage.getItem(WEB_SESSION_KEY);
}

export async function clearStoredSessionToken(): Promise<void> {
  if (isDesktopRuntime()) await invoke("clear_session_token");
  else sessionStorage.removeItem(WEB_SESSION_KEY);
  localStorage.removeItem(WEB_SESSION_KEY);
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
    request: {
      room_id: roomId,
      stun_server: runtimeStunServer,
    },
  });
}

export async function runReservedMatchProbe(request: {
  room_id: string;
  game_id: string;
  local_user_id: string;
  peer_user_id: string;
  role: "host" | "guest";
  peer_endpoint: string;
  peer_reflexive_endpoint?: string;
  peer_nonce: string;
  frame_count?: number;
  timeout_ms?: number;
}): Promise<MatchProbeReport> {
  if (!isDesktopRuntime()) throw new Error("LAN match probe requires the desktop client");
  return invoke<MatchProbeReport>("run_reserved_match_probe", { request });
}

export async function runRelayMatchProbe(request: {
  relay_url: string;
  ticket: {
    room_id: string;
    user_id: string;
    expires_at: number;
    signature: string;
  };
  room_id: string;
  game_id: string;
  local_user_id: string;
  peer_user_id: string;
  role: "host" | "guest";
  local_nonce: string;
  peer_nonce: string;
  frame_count?: number;
  timeout_ms?: number;
}): Promise<MatchProbeReport> {
  if (!isDesktopRuntime()) throw new Error("Relay match probe requires the desktop client");
  return invoke<MatchProbeReport>("run_relay_match_probe_command", { request });
}

export async function cancelMatchProbe(roomId: string): Promise<void> {
  if (!isDesktopRuntime()) return;
  await invoke("cancel_match_probe", { roomId });
}
