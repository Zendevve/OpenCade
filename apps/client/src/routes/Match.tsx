import { useQuery } from "@tanstack/react-query";
import type { MatchEndpointPayload, MatchProbeCompletedPayload } from "@opencade/protocol";
import { api } from "../lib/api";
import { downloadMatchReport } from "../lib/report";
import { useLanMatchProbe } from "../lib/useLanMatchProbe";
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
  const { localEndpoint, probeReport, probeError, isResetting, retry } = useLanMatchProbe({
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
  const steps = ["connecting", "playing", "finished"];
  const active = Math.max(0, steps.indexOf(state));
  const heading =
    state === "connecting"
      ? "Establishing peer session"
      : state === "playing"
        ? "Match in progress"
        : state === "finished"
          ? "Match complete"
          : `Room ${state}`;
  const completedRoom = room.data?.state === "finished" ? room.data : undefined;
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
          Direct UDP verified: {probeReport.frames_received} frames in {probeReport.elapsed_ms} ms ·
          transcript {probeReport.transcript_checksum}
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
      <div className="match-actions">
        {probeError && (
          <button className="secondary" disabled={isResetting} onClick={() => void retry()}>
            {isResetting ? "Resetting LAN probe…" : "Retry LAN probe"}
          </button>
        )}
        {completedRoom && probeReport && (
          <button
            className="secondary"
            onClick={() => downloadMatchReport(completedRoom, probeReport)}
          >
            Export report
          </button>
        )}
        <button className="secondary" onClick={onDone}>
          Return to games
        </button>
      </div>
    </section>
  );
}
