import { invoke } from "@tauri-apps/api/core";

export type NatType = "open" | "cone" | "symmetric" | "blocked" | "unknown";

export type NetworkDiagnostics = {
  nat: NatType | string;
  rtt_ms: number | null;
  loss: number;
  jitter_ms: number;
  relay_reachable: boolean;
  stun_reachable: boolean;
};

/**
 * Typed wrapper around the Tauri `network_test` command.
 * Keeps backward compatibility: callers that only read `nat`, `rtt_ms`,
 * and `relay_reachable` continue to work while new fields are available.
 */
export async function networkTest(): Promise<NetworkDiagnostics> {
  return invoke<NetworkDiagnostics>("network_test");
}

// Backwards-compatible alias for existing callers that import `runNetworkTest` from native.ts
export const runDiagnostics = networkTest;
