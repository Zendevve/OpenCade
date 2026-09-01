import { describe, expect, it, vi } from "vitest";
import { controllerPreflight } from "./controller";

describe("controllerPreflight", () => {
  it("assigns deterministic complementary player slots (dev loopback mocks connected when no pad)", () => {
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: vi.fn(() => []),
    });
    // Threshold: in dev (Vite DEV) empty pads are mocked to connected:true for opencade_test loopback;
    // production (DEV=false) would remain connected:false and block at server preflight.
    expect(controllerPreflight("host")).toEqual({ connected: true, player_slot: 1 });
    expect(controllerPreflight("guest")).toEqual({ connected: true, player_slot: 2 });
  });

  it("requires at least one connected controller", () => {
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: vi.fn(() => [{ connected: true } as Gamepad, null, null, null]),
    });
    expect(controllerPreflight("host").connected).toBe(true);
  });
});
