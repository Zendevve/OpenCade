import { describe, expect, it } from "vitest";
import { buildMatchReport } from "./report.js";

describe("buildMatchReport", () => {
  it("contains room correlation without credentials or local paths", () => {
    const report = buildMatchReport(
      {
        id: "room-123",
        game_id: "sfiii3",
        host_id: "host",
        guest_id: "guest",
        state: "finished",
      },
      new Date("2026-08-23T12:00:00Z")
    );
    const serialized = JSON.stringify(report);
    expect(report.exported_at).toBe("2026-08-23T12:00:00.000Z");
    expect(serialized).toContain("room-123");
    expect(serialized).not.toContain("token");
    expect(serialized).not.toContain("rom_path");
  });

  it("includes useful UDP evidence without private session material", () => {
    const report = buildMatchReport(
      {
        id: "room-123",
        game_id: "sfiii3",
        host_id: "host",
        guest_id: "guest",
        state: "finished",
      },
      new Date("2026-08-23T12:00:00Z"),
      {
        room_id: "room-123",
        local_user_id: "host",
        peer_user_id: "guest",
        role: "host",
        transport: "direct_udp",
        frames_sent: 60,
        frames_received: 60,
        transcript_checksum: "0123456789abcdef",
        elapsed_ms: 240,
      }
    );

    expect(report.probe).toEqual({
      transport: "direct_udp",
      frames_sent: 60,
      frames_received: 60,
      transcript_checksum: "0123456789abcdef",
      elapsed_ms: 240,
    });
    expect(JSON.stringify(report.probe)).not.toContain("user_id");
    expect(JSON.stringify(report.probe)).not.toContain("nonce");
    expect(JSON.stringify(report.probe)).not.toContain("endpoint");
  });
});
