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
};

export type ReadinessAssessment = {
  checks: ReadinessCheck[];
  canContinue: boolean;
  readyRequired: number;
  requiredTotal: number;
};

export function assessMatchReadiness(input: ReadinessInput): ReadinessAssessment {
  const checks: ReadinessCheck[] = [
    {
      id: "desktop",
      title: "Desktop runtime",
      detail: input.desktop
        ? "Native diagnostics and safe emulator launch are available."
        : "Install and open the desktop client before joining a playable match.",
      state: input.desktop ? "ready" : "blocked",
      required: true,
    },
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
  return {
    id: "native_port",
    title: "Match port",
    detail: input.preflight.native_port_available
      ? "The native netplay port is available."
      : "Close the process using the netplay port, then retry.",
    state: input.preflight.native_port_available ? "ready" : "blocked",
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
