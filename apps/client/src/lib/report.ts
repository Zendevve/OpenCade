import type { MatchReport, RoomPayload } from "@opencade/protocol";
import type { MatchProbeReport } from "./native";

export function buildMatchReport(
  room: RoomPayload,
  probe: MatchProbeReport,
  now = new Date()
): MatchReport {
  return {
    schema_version: 1,
    exported_at: now.toISOString(),
    room: { id: room.id, game_id: room.game_id, state: room.state },
    probe: {
      role: probe.role,
      transport: probe.transport,
      frames_sent: probe.frames_sent,
      frames_received: probe.frames_received,
      transcript_checksum: probe.transcript_checksum,
      elapsed_ms: probe.elapsed_ms,
      nat: probe.nat,
      candidate: probe.candidate,
      punch_attempts: probe.punch_attempts,
    },
    client: {
      platform: typeof navigator === "undefined" ? "unknown" : navigator.platform,
      user_agent: typeof navigator === "undefined" ? "unknown" : navigator.userAgent,
    },
  };
}

export function downloadMatchReport(room: RoomPayload, probe: MatchProbeReport): void {
  const report = buildMatchReport(room, probe, new Date());
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(report, null, 2)], { type: "application/json" })
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = `opencade-match-${room.id.slice(0, 8)}.json`;
  link.click();
  URL.revokeObjectURL(url);
}
