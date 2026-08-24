import type { RoomPayload } from "@opencade/protocol";

export type MatchReport = {
  schema_version: 1;
  exported_at: string;
  room: RoomPayload;
  client: { platform: string; user_agent: string };
};

export function buildMatchReport(room: RoomPayload, now = new Date()): MatchReport {
  return {
    schema_version: 1,
    exported_at: now.toISOString(),
    room,
    client: {
      platform: typeof navigator === "undefined" ? "unknown" : navigator.platform,
      user_agent: typeof navigator === "undefined" ? "unknown" : navigator.userAgent,
    },
  };
}

export function downloadMatchReport(room: RoomPayload): void {
  const report = buildMatchReport(room);
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(report, null, 2)], { type: "application/json" })
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = `opencade-match-${room.id.slice(0, 8)}.json`;
  link.click();
  URL.revokeObjectURL(url);
}
