import type { RoomPayload } from "@opencade/protocol";
import type { MatchProbeReport } from "./native";

export type MatchProbeEvidence = Pick<
  MatchProbeReport,
  "transport" | "frames_sent" | "frames_received" | "transcript_checksum" | "elapsed_ms"
>;

export type MatchReport = {
  schema_version: 1;
  exported_at: string;
  room: RoomPayload;
  probe?: MatchProbeEvidence;
  client: { platform: string; user_agent: string };
};

export function buildMatchReport(
  room: RoomPayload,
  now = new Date(),
  probe?: MatchProbeReport
): MatchReport {
  return {
    schema_version: 1,
    exported_at: now.toISOString(),
    room,
    ...(probe && {
      probe: {
        transport: probe.transport,
        frames_sent: probe.frames_sent,
        frames_received: probe.frames_received,
        transcript_checksum: probe.transcript_checksum,
        elapsed_ms: probe.elapsed_ms,
      },
    }),
    client: {
      platform: typeof navigator === "undefined" ? "unknown" : navigator.platform,
      user_agent: typeof navigator === "undefined" ? "unknown" : navigator.userAgent,
    },
  };
}

export function downloadMatchReport(room: RoomPayload, probe?: MatchProbeReport): void {
  const report = buildMatchReport(room, new Date(), probe);
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(report, null, 2)], { type: "application/json" })
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = `opencade-match-${room.id.slice(0, 8)}.json`;
  link.click();
  URL.revokeObjectURL(url);
}
