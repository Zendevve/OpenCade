import { describe, expect, it } from "vitest";
import { reconnectDelay } from "./ws.js";

describe("reconnectDelay", () => {
  it("backs off exponentially and caps at 30 seconds", () => {
    expect(reconnectDelay(0)).toBe(500);
    expect(reconnectDelay(3)).toBe(4_000);
    expect(reconnectDelay(20)).toBe(30_000);
  });
});
