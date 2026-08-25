import { describe, expect, it } from "vitest";
import { assessMatchReadiness, type ReadinessInput } from "./readiness";

const readyInput = (): ReadinessInput => ({
  desktop: true,
  controlPlaneReady: true,
  preflightStatus: "success",
  preflight: {
    adapter: "retroarch_fbneo",
    emulator_version: "1.22.0",
    executable_sha256: "a".repeat(64),
    core_sha256: "b".repeat(64),
    content_sha256: "c".repeat(64),
    native_port_available: true,
  },
  networkStatus: "success",
  network: { nat: "mapped", rtt_ms: 24, relay_reachable: true },
});

describe("assessMatchReadiness", () => {
  it("allows entry only when every required capability is ready", () => {
    const assessment = assessMatchReadiness(readyInput());
    expect(assessment.canContinue).toBe(true);
    expect(assessment.readyRequired).toBe(assessment.requiredTotal);
  });

  it("fails closed outside the desktop runtime", () => {
    const assessment = assessMatchReadiness({ ...readyInput(), desktop: false });
    expect(assessment.canContinue).toBe(false);
    expect(assessment.checks.find((check) => check.id === "desktop")?.state).toBe("blocked");
  });

  it("blocks a busy native port", () => {
    const input = readyInput();
    if (!input.preflight) throw new Error("fixture preflight is missing");
    input.preflight.native_port_available = false;
    const assessment = assessMatchReadiness(input);
    expect(assessment.canContinue).toBe(false);
    expect(assessment.checks.find((check) => check.id === "native_port")?.state).toBe("blocked");
  });

  it("keeps uncertain internet fallback advisory for same-LAN play", () => {
    const assessment = assessMatchReadiness({
      ...readyInput(),
      network: { nat: "blocked", rtt_ms: null, relay_reachable: false },
    });
    expect(assessment.canContinue).toBe(true);
    expect(assessment.checks.find((check) => check.id === "network")?.state).toBe("warning");
  });
});
