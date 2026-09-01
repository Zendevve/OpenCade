import type { NetworkDiagnostics, RetroarchPreflight } from "./native";

export type ReadinessState = "pending" | "ready" | "warning" | "blocked";

export type ReadinessCheck = {
  id: "desktop" | "control_plane" | "game_runtime" | "native_port" | "network";
  title: string;
  detail: string;
  state: ReadinessState;
  required: boolean;
};

export type ReadinessInput = {
  desktop: boolean;
  controlPlaneReady: boolean;
  preflightStatus: "pending" | "success" | "error";
  preflight?: RetroarchPreflight;
  networkStatus: "pending" | "success" | "error";
  network?: NetworkDiagnostics;
  gameId?: string;
  isDev?: boolean;
};

export type ReadinessAssessment = {
  checks: ReadinessCheck[];
  canContinue: boolean;
  readyRequired: number;
  requiredTotal: number;
};

/**
 * Threshold behavior:
 * - release (isDev=false or gameId != opencade_test): desktop/game_runtime/native_port are required and block.
 * - dev loopback (isDev=true && gameId === "opencade_test"): desktop/game_runtime/native_port downgrade to warning (required=false)
 *   so `pnpm dev` in a plain browser + `cargo run -p opencade-server` is playable locally without Tauri/WebView2.
 *   Real RetroArch native launch still fail-closed on server for non-dev and non-test games.
 *   Network is advisory in both modes. Controller fail-closed via server preflight.
 */
export function assessMatchReadiness(input: ReadinessInput): ReadinessAssessment {
  const desktopCheck: ReadinessCheck = input.desktop
    ? {
        id: "desktop",
        title: "Desktop runtime",
        detail: "Native diagnostics and safe emulator launch are available.",
        state: "ready",
        required: true,
      }
    : input.isDev && input.gameId === "opencade_test"
      ? {
          id: "desktop",
          title: "Desktop runtime",
          detail:
            "Dev browser: Tauri not detected, but opencade_test can continue on loopback (mocked launch). Use Tauri for real RetroArch netplay.",
          state: "warning",
          required: false,
        }
      : {
          id: "desktop",
          title: "Desktop runtime",
          detail: "Install and open the desktop client before joining a playable match.",
          state: "blocked",
          required: true,
        };
  const checks: ReadinessCheck[] = [
    desktopCheck,
    {
      id: "control_plane",
      title: "Control plane",
      detail: input.controlPlaneReady
        ? "The authenticated game service is reachable."
        : "Reconnect to the OpenFight service, then run the checks again.",
      state: input.controlPlaneReady ? "ready" : "blocked",
      required: true,
    },
    runtimeCheck(input),
    portCheck(input),
    networkCheck(input),
  ];
  const required = checks.filter((check) => check.required);
  const readyRequired = required.filter((check) => check.state === "ready").length;
  return {
    checks,
    canContinue: readyRequired === required.length,
    readyRequired,
    requiredTotal: required.length,
  };
}

function runtimeCheck(input: ReadinessInput): ReadinessCheck {
  if (!input.desktop) {
    if (input.isDev && input.gameId === "opencade_test") {
      return {
        id: "game_runtime",
        title: "Game runtime",
        detail:
          "Dev browser: runtime mocked for opencade_test loopback (proof-of-match). Use desktop client for real RetroArch fingerprints.",
        state: "warning",
        required: false,
      };
    }
    return {
      id: "game_runtime",
      title: "Game runtime",
      detail: "Runtime validation becomes available in the desktop client.",
      state: "blocked",
      required: true,
    };
  }
  if (input.preflightStatus === "pending") {
    return {
      id: "game_runtime",
      title: "Game runtime",
      detail: "Checking the emulator, core, and local game content…",
      state: "pending",
      required: true,
    };
  }
  if (input.preflightStatus === "error" || !input.preflight) {
    if (input.isDev && input.gameId === "opencade_test") {
      return {
        id: "game_runtime",
        title: "Game runtime",
        detail:
          "Dev fixture: RetroArch preflight did not verify, but opencade_test can continue on loopback (proof-of-match mock). Re-run with the fixture installed for full verification.",
        state: "warning",
        required: false,
      };
    }
    return {
      id: "game_runtime",
      title: "Game runtime",
      detail: "Check the configured emulator, core, and game content, then retry.",
      state: "blocked",
      required: true,
    };
  }
  return {
    id: "game_runtime",
    title: "Game runtime",
    detail: "The emulator, core, and local game content produced valid fingerprints.",
    state: "ready",
    required: true,
  };
}

function portCheck(input: ReadinessInput): ReadinessCheck {
  if (!input.desktop || input.preflightStatus === "error") {
    if (input.isDev && input.gameId === "opencade_test") {
      return {
        id: "native_port",
        title: "Match port",
        detail:
          "Dev fixture: native port validation is advisory for opencade_test on loopback; real netplay will still require port 55435.",
        state: "warning",
        required: false,
      };
    }
    return {
      id: "native_port",
      title: "Match port",
      detail: "Port validation requires a valid desktop game runtime.",
      state: "blocked",
      required: true,
    };
  }
  if (input.preflightStatus === "pending" || !input.preflight) {
    return {
      id: "native_port",
      title: "Match port",
      detail: "Waiting for the local runtime check…",
      state: "pending",
      required: true,
    };
  }
  if (!input.preflight.native_port_available) {
    if (input.isDev && input.gameId === "opencade_test") {
      return {
        id: "native_port",
        title: "Match port",
        detail:
          "Dev fixture: native port 55435 is busy, but opencade_test can continue on loopback (warning only). Close the holder for real matches.",
        state: "warning",
        required: false,
      };
    }
    return {
      id: "native_port",
      title: "Match port",
      detail: "Close the process using the netplay port, then retry.",
      state: "blocked",
      required: true,
    };
  }
  return {
    id: "native_port",
    title: "Match port",
    detail: "The native netplay port is available.",
    state: "ready",
    required: true,
  };
}

function networkCheck(input: ReadinessInput): ReadinessCheck {
  if (!input.desktop || input.networkStatus === "error") {
    return {
      id: "network",
      title: "Network fallback",
      detail: "Network diagnostics are unavailable. Same-LAN play may still work.",
      state: "warning",
      required: false,
    };
  }
  if (input.networkStatus === "pending" || !input.network) {
    return {
      id: "network",
      title: "Network fallback",
      detail: "Checking NAT reachability and relay availability…",
      state: "pending",
      required: false,
    };
  }
  const reachable = input.network.nat !== "blocked" || input.network.relay_reachable;
  return {
    id: "network",
    title: "Network fallback",
    detail: reachable
      ? "At least one diagnostic network path is available."
      : "No internet path was verified. You can continue with a same-LAN opponent.",
    state: reachable ? "ready" : "warning",
    required: false,
  };
}
