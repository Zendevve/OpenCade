import { describe, expect, it, vi } from "vitest";
import { controllerPreflight } from "./controller";

describe("controllerPreflight", () => {
  it("assigns deterministic complementary player slots", () => {
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: vi.fn(() => []),
    });
    expect(controllerPreflight("host")).toEqual({ connected: false, player_slot: 1 });
    expect(controllerPreflight("guest")).toEqual({ connected: false, player_slot: 2 });
  });

  it("requires at least one connected controller", () => {
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: vi.fn(() => [{ connected: true } as Gamepad, null, null, null]),
    });
    expect(controllerPreflight("host").connected).toBe(true);
  });
});
