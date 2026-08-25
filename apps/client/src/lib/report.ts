import type {
  AlphaFailureReport,
  AlphaFailureStage,
  MatchReport,
  MatchReportCompatibility,
  MatchReportRole,
  MatchReportTransport,
  RoomPayload,
} from "@opencade/protocol";
import type { MatchProbeReport, RetroarchMatchLaunch } from "./native";

export function buildMatchReport(
  room: RoomPayload,
  probe: MatchProbeReport,
  now = new Date(),
  compatibility?: MatchReportCompatibility
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
      candidate: probe.transport === "relay" ? null : probe.candidate,
      punch_attempts: probe.transport === "relay" ? null : probe.punch_attempts,
    },
    client: {
      platform: typeof navigator === "undefined" ? "unknown" : navigator.platform,
      user_agent: typeof navigator === "undefined" ? "unknown" : navigator.userAgent,
    },
    compatibility: compatibility ?? null,
  };
}

export function downloadMatchReport(
  room: RoomPayload,
  probe: MatchProbeReport,
  playable?: RetroarchMatchLaunch
): void {
  const compatibility: MatchReportCompatibility | undefined = playable
    ? {
        adapter: playable.adapter,
        emulator_version: playable.fingerprint.retroarch_version ?? null,
        executable_sha256: playable.fingerprint.executable_sha256,
        core_sha256: playable.fingerprint.core_sha256,
        content_sha256: playable.fingerprint.content_sha256,
      }
    : undefined;
  downloadJson(
    `opencade-match-${room.id.slice(0, 8)}.json`,
    buildMatchReport(room, probe, new Date(), compatibility)
  );
}

export type AlphaFailureEvidence = {
  stage: AlphaFailureStage;
  error_code: string;
  transport?: MatchReportTransport;
};

export function buildAlphaFailureReport(
  room: RoomPayload,
  role: MatchReportRole,
  failure: AlphaFailureEvidence,
  now = new Date()
): AlphaFailureReport {
  if (!/^[a-z0-9_]{3,64}$/.test(failure.error_code)) {
    throw new Error("Failure evidence requires a stable error code");
  }
  return {
    schema_version: 1,
    kind: "attempt_failure",
    exported_at: now.toISOString(),
    room: { id: room.id, game_id: room.game_id, state: room.state },
    role,
    stage: failure.stage,
    error_code: failure.error_code,
    transport: failure.transport ?? null,
    client: {
      platform: typeof navigator === "undefined" ? "unknown" : navigator.platform,
      user_agent: typeof navigator === "undefined" ? "unknown" : navigator.userAgent,
    },
  };
}

export function downloadAlphaFailureReport(
  room: RoomPayload,
  role: MatchReportRole,
  failure: AlphaFailureEvidence
): void {
  downloadJson(
    `opencade-failure-${room.id.slice(0, 8)}.json`,
    buildAlphaFailureReport(room, role, failure)
  );
}

function downloadJson(filename: string, value: unknown): void {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" })
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
