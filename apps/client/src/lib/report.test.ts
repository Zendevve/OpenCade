import { describe, expect, it } from "vitest";
import { buildAlphaFailureReport, buildMatchReport } from "./report.js";

describe("buildMatchReport", () => {
  const probe = {
    room_id: "room-123",
    local_user_id: "host",
    peer_user_id: "guest",
    role: "host" as const,
    transport: "direct_udp" as const,
    frames_sent: 64,
    frames_received: 60,
    transcript_checksum: "0123456789abcdef",
    elapsed_ms: 240,
    nat: "mapped" as const,
    candidate: "reflexive" as const,
    punch_attempts: 2,
  };

  it("contains room correlation without identities, credentials, or local paths", () => {
    const report = buildMatchReport(
      {
        id: "room-123",
        game_id: "sfiii3",
        host_id: "host",
        guest_id: "guest",
        state: "finished",
      },
      probe,
      new Date("2026-08-23T12:00:00Z")
    );
    const serialized = JSON.stringify(report);
    expect(report.exported_at).toBe("2026-08-23T12:00:00.000Z");
    expect(serialized).toContain("room-123");
    expect(serialized).not.toContain("local_user_id");
    expect(serialized).not.toContain("peer_user_id");
    expect(serialized).not.toContain("host_id");
    expect(serialized).not.toContain("guest_id");
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
      probe,
      new Date("2026-08-23T12:00:00Z")
    );

    expect(report.probe).toEqual({
      role: "host",
      transport: "direct_udp",
      frames_sent: 64,
      frames_received: 60,
      transcript_checksum: "0123456789abcdef",
      elapsed_ms: 240,
      nat: "mapped",
      candidate: "reflexive",
      punch_attempts: 2,
    });
    expect(JSON.stringify(report.probe)).not.toContain("user_id");
    expect(JSON.stringify(report.probe)).not.toContain("nonce");
    expect(JSON.stringify(report.probe)).not.toContain("endpoint");
  });

  it("records relay evidence without inventing direct candidate data", () => {
    const report = buildMatchReport(
      {
        id: "room-123",
        game_id: "sfiii3",
        host_id: "host",
        guest_id: "guest",
        state: "finished",
      },
      { ...probe, transport: "relay" },
      new Date("2026-08-23T12:00:00Z")
    );

    expect(report.probe.transport).toBe("relay");
    expect(report.probe.candidate).toBeNull();
    expect(report.probe.punch_attempts).toBeNull();
  });

  it("adds compatibility hashes without exposing emulator paths", () => {
    const report = buildMatchReport(
      {
        id: "room-123",
        game_id: "sfiii3",
        host_id: "host",
        guest_id: "guest",
        state: "finished",
      },
      probe,
      new Date("2026-08-23T12:00:00Z"),
      {
        adapter: "retroarch_fbneo",
        emulator_version: "1.22.0",
        executable_sha256: "a".repeat(64),
        core_sha256: "b".repeat(64),
        content_sha256: "c".repeat(64),
      }
    );

    expect(report.compatibility?.adapter).toBe("retroarch_fbneo");
    expect(JSON.stringify(report.compatibility)).not.toContain("C:\\");
    expect(JSON.stringify(report.compatibility)).not.toContain("path");
  });
});

describe("buildAlphaFailureReport", () => {
  const room = {
    id: "room-failed",
    game_id: "sfiii3",
    host_id: "host",
    guest_id: "guest",
    state: "connecting" as const,
  };

  it("records a stable failure stage without identities or diagnostic messages", () => {
    const report = buildAlphaFailureReport(
      room,
      "host",
      {
        stage: "relay",
        error_code: "relay_probe_failed",
        transport: "relay",
      },
      new Date("2026-08-24T12:00:00Z")
    );
    const serialized = JSON.stringify(report);

    expect(report.kind).toBe("attempt_failure");
    expect(report.stage).toBe("relay");
    expect(report.transport).toBe("relay");
    expect(serialized).not.toContain("host_id");
    expect(serialized).not.toContain("guest_id");
    expect(serialized).not.toContain("endpoint");
    expect(serialized).not.toContain("C:\\");
  });

  it("rejects free-form text that could leak local diagnostics", () => {
    expect(() =>
      buildAlphaFailureReport(room, "host", {
        stage: "native_launch",
        error_code: "Failed at C:\\Users\\tester",
      })
    ).toThrow("stable error code");
  });
});
