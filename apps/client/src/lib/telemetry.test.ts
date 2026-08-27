import { describe, expect, it } from "vitest";
import {
  TELEMETRY_CONSENT_KEY,
  createProductEvent,
  getTelemetryConsent,
  setTelemetryConsent,
} from "./telemetry.js";

function memoryStorage(): Pick<Storage, "getItem" | "setItem"> {
  const values = new Map<string, string>();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => void values.set(key, value),
  };
}

describe("product telemetry", () => {
  it("defaults to no consent and persists either explicit choice", () => {
    const storage = memoryStorage();
    expect(getTelemetryConsent(storage)).toBeNull();
    expect(setTelemetryConsent(true, storage)).toBe(true);
    expect(getTelemetryConsent(storage)).toBe(true);
    expect(setTelemetryConsent(false, storage)).toBe(true);
    expect(getTelemetryConsent(storage)).toBe(false);
    expect(storage.getItem(TELEMETRY_CONSENT_KEY)).toBe("declined");
  });

  it("builds only the closed anonymous event payload", () => {
    const payload = createProductEvent(
      "00000000-0000-4000-8000-000000000001",
      "readiness_blocked",
      "sfiii3",
      ["game_runtime"],
      "00000000-0000-4000-8000-000000000002"
    );
    expect(payload).toEqual({
      event_id: "00000000-0000-4000-8000-000000000002",
      anonymous_session_id: "00000000-0000-4000-8000-000000000001",
      event: "readiness_blocked",
      game_id: "sfiii3",
      blocked_checks: ["game_runtime"],
    });
    expect(payload).not.toHaveProperty("user_id");
    expect(payload).not.toHaveProperty("properties");
  });
});
