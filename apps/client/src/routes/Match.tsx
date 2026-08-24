import { useQuery } from "@tanstack/react-query";
import type { MatchEndpointPayload, MatchProbeCompletedPayload } from "@opencade/protocol";
import { api } from "../lib/api";
import { downloadAlphaFailureReport, downloadMatchReport } from "../lib/report";
import { useLanMatchProbe } from "../lib/useLanMatchProbe";
import { usePlayableMatch } from "../lib/usePlayableMatch";
import type { OpenCadeSocket } from "../lib/ws";

export default function Match({
  token,
  userId,
  roomId,
  socket,
  peerEndpoint,
  peerCompletion,
  onProbeRetry,
  onDone,
}: {
  token: string;
  userId: string;
  roomId: string;
  socket: OpenCadeSocket | null;
  peerEndpoint?: MatchEndpointPayload;
  peerCompletion?: MatchProbeCompletedPayload;
  onProbeRetry: () => void;
  onDone: () => void;
}) {
  const room = useQuery({
    queryKey: ["room", roomId],
    queryFn: () => api.room(token, roomId),
    refetchInterval: 2_000,
  });
  const { localEndpoint, probeReport, probeError, probeFailure, isResetting, retry } =
    useLanMatchProbe({
      token,
      userId,
      roomId,
      room: room.data,
      socket,
      peerEndpoint,
      peerCompletion,
      onRetry: onProbeRetry,
    });
  const state = room.data?.state ?? "connecting";
  const { coordinator, participants, playableMatch, resetCoordinator } = usePlayableMatch({
    token,
    userId,
    roomId,
    room: room.data,
    localEndpoint,
    peerEndpoint,
    probeReport,
    peerCompletion,
  });
  const steps = ["connecting", "playing", "finished"];
  const active = Math.max(0, steps.indexOf(state));
  const heading =
    coordinator.phase === "relay_probe_only"
      ? "Relay readiness verified"
      : state === "connecting"
        ? "Establishing peer session"
        : state === "playing"
          ? "Match in progress"
          : state === "finished"
            ? "Match complete"
            : `Room ${state}`;
  const completedRoom = room.data?.state === "finished" ? room.data : undefined;
  const failureRoom = room.data;
  const failureEvidence = playableMatch.isError
    ? {
        stage: "native_launch" as const,
        error_code: "native_launch_failed",
        transport: probeReport?.transport,
      }
    : coordinator.phase === "failed" && coordinator.error?.includes("transcript")
      ? {
          stage: "peer_transcript" as const,
          error_code: "peer_transcript_mismatch",
          transport: probeReport?.transport,
        }
      : probeFailure;
  return (
    <section className="match-stage">
      <p className="eyebrow">Room {roomId.slice(0, 8)}</p>
      <h2>{heading}</h2>
      <div className="match-orbit" aria-hidden="true">
        <span>YOU</span>
        <i />
        <span>PEER</span>
      </div>
      <ol className="match-steps" aria-label="Match connection progress">
        {steps.map((step, index) => (
          <li className={index <= active ? "active" : ""} key={step}>
            {step}
          </li>
        ))}
      </ol>
      {localEndpoint && !probeReport && (
        <p className="status-copy" role="status">
          LAN endpoint reserved at {localEndpoint.endpoint}
        </p>
      )}
      {probeReport && (
        <p className="status-copy" role="status">
          {probeReport.transport === "relay" ? "Authenticated relay" : "Direct UDP"} verified:{" "}
          {probeReport.frames_received} frames in {probeReport.elapsed_ms} ms · transcript{" "}
          {probeReport.transcript_checksum}
        </p>
      )}
      {coordinator.phase === "relay_probe_only" && (
        <p className="status-copy" role="status">
          The readiness probe passed, but this UDP route is not a usable RetroArch TCP route. Native
          gameplay is limited to a verified same-LAN host candidate for now.
        </p>
      )}
      {coordinator.error && !playableMatch.isError && (
        <p className="form-error" role="alert">
          {coordinator.error}
        </p>
      )}
      {probeError && (
        <p className="form-error" role="alert">
          {probeError}
        </p>
      )}
      {room.isError && (
        <p className="form-error" role="alert">
          {room.error.message}
        </p>
      )}
      {playableMatch.isError && (
        <p className="form-error" role="alert">
          {playableMatch.error.message}
        </p>
      )}
      {playableMatch.data && (
        <p className="status-copy" role="status">
          RetroArch netplay launched · PID {playableMatch.data.pid} · content{" "}
          {playableMatch.data.fingerprint.content_sha256.slice(0, 12)}
        </p>
      )}
      <div className="match-actions">
        {(probeError || coordinator.phase === "relay_probe_only") && (
          <button
            className="secondary"
            disabled={isResetting}
            onClick={() => {
              resetCoordinator();
              void retry();
            }}
          >
            {isResetting
              ? "Resetting LAN probe…"
              : coordinator.phase === "relay_probe_only"
                ? "Retry direct UDP"
                : "Retry LAN probe"}
          </button>
        )}
        {completedRoom && probeReport && (
          <button
            className="secondary"
            onClick={() => downloadMatchReport(completedRoom, probeReport, playableMatch.data)}
          >
            Export report
          </button>
        )}
        {failureRoom && participants && failureEvidence && (
          <button
            className="secondary"
            onClick={() =>
              downloadAlphaFailureReport(failureRoom, participants.role, failureEvidence)
            }
          >
            Export failure evidence
          </button>
        )}
        {coordinator.phase === "ready" && participants && !playableMatch.data && (
          <button
            className="primary"
            disabled={playableMatch.isPending}
            onClick={() => playableMatch.mutate()}
          >
            {playableMatch.isPending ? "Launching RetroArch…" : "Launch playable alpha"}
          </button>
        )}
        <button className="secondary" onClick={onDone}>
          Return to games
        </button>
      </div>
    </section>
  );
}
