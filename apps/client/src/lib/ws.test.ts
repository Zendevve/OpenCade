import { describe, expect, it } from "vitest";
import { reconnectDelay } from "./ws.js";

describe("reconnectDelay", () => {
  it("uses bounded jitter while backing off exponentially", () => {
    expect(reconnectDelay(0, () => 0)).toBe(250);
    expect(reconnectDelay(0, () => 1)).toBe(500);
    expect(reconnectDelay(3, () => 0)).toBe(2_000);
    expect(reconnectDelay(3, () => 1)).toBe(4_000);
    expect(reconnectDelay(20, () => 0)).toBe(15_000);
    expect(reconnectDelay(20, () => 1)).toBe(30_000);
  });
});
