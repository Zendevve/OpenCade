import { afterEach, describe, expect, it, vi } from "vitest";
import { api, configureApiBase } from "./api.js";

describe("runtime API configuration", () => {
  afterEach(() => {
    configureApiBase("http://localhost:8080");
    vi.unstubAllGlobals();
  });

  it("routes requests through a validated runtime origin", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ type: "games.list", payload: { games: [] } }),
    });
    vi.stubGlobal("fetch", fetchMock);
    configureApiBase("https://alpha.example.com/");

    await api.games("session-token");

    expect(fetchMock).toHaveBeenCalledWith(
      "https://alpha.example.com/api/v1/games",
      expect.objectContaining({ headers: expect.any(Headers) })
    );
  });

  it("rejects non-http origins and embedded credentials", () => {
    expect(() => configureApiBase("file:///tmp/server")).toThrow(/HTTP or HTTPS/);
    expect(() => configureApiBase("https://user:pass@example.com")).toThrow(/credentials/);
    expect(() => configureApiBase("http://192.168.1.10:8080")).toThrow(/must use HTTPS/);
  });
});
