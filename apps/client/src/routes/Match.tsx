import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { MatchEndpointPayload, MatchProbeCompletedPayload } from "@opencade/protocol";
import { api } from "../lib/api";
import {
  buildAlphaFailureReport,
  buildMatchReport,
  compatibilityFromLaunch,
  downloadAlphaFailureReport,
  downloadMatchReport,
} from "../lib/report";
import { useLanMatchProbe } from "../lib/useLanMatchProbe";
import { usePlayableMatch } from "../lib/usePlayableMatch";
import { stopGame } from "../lib/native";
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
  const {
    coordinator,
    participants,
    playableMatch,
    snapshot,
    preflightPending,
    launchBarrierPending,
    canLaunch,
    retryPreflight,
    resetCoordinator,
  } = usePlayableMatch({
    token,
    userId,
    roomId,
    room: room.data,
    localEndpoint,
    peerEndpoint,
    probeReport,
    peerCompletion,
  });
  const [now, setNow] = useState(Date.now());
  const [isLeaving, setIsLeaving] = useState(false);
  const [leaveError, setLeaveError] = useState("");
  const [receiptCopied, setReceiptCopied] = useState(false);
  const [receiptCopyError, setReceiptCopyError] = useState("");
  const receiptKey = useRef(crypto.randomUUID());
  const uploadedSuccess = useRef(false);
  const uploadedFailure = useRef(false);
  useEffect(() => {
    if (!snapshot?.barrier.launch_at || state !== "connecting") return;
    const deadline = new Date(snapshot.barrier.launch_at).getTime();
    if (deadline <= Date.now()) return;
    const timer = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, [snapshot?.barrier.launch_at, state]);
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
        native_route: snapshot?.route,
      }
    : coordinator.phase === "failed" && coordinator.error?.includes("transcript")
      ? {
          stage: "peer_transcript" as const,
          error_code: "peer_transcript_mismatch",
          transport: probeReport?.transport,
        }
      : probeFailure;
  const evidenceUpload = useMutation({
    mutationFn: async () => {
      if (!completedRoom || !probeReport || !playableMatch.data) {
        throw new Error("Completed match evidence is unavailable");
      }
      const report = buildMatchReport(
        completedRoom,
        probeReport,
        new Date(),
        compatibilityFromLaunch(playableMatch.data),
        playableMatch.data.native_route
      );
      return api.submitEvidence(token, report);
    },
    retry: 2,
    onMutate: () => {
      uploadedSuccess.current = true;
    },
    onError: () => {
      uploadedSuccess.current = false;
    },
  });
  useEffect(() => {
    if (
      !completedRoom ||
      !probeReport ||
      !playableMatch.data ||
      uploadedSuccess.current ||
      evidenceUpload.status !== "idle"
    )
      return;
    evidenceUpload.mutate();
  }, [completedRoom, evidenceUpload, playableMatch.data, probeReport]);
  useEffect(() => {
    if (!failureRoom || !participants || !failureEvidence || uploadedFailure.current) return;
    uploadedFailure.current = true;
    const report = buildAlphaFailureReport(failureRoom, participants.role, failureEvidence);
    void api.submitEvidence(token, report).catch(() => {
      uploadedFailure.current = false;
    });
  }, [failureEvidence, failureRoom, participants, token]);
  const launchSeconds = snapshot?.barrier.launch_at
    ? Math.max(0, Math.ceil((new Date(snapshot.barrier.launch_at).getTime() - now) / 1000))
    : undefined;
  const receipt = useMutation({
    mutationFn: () => api.createMatchReceipt(token, roomId, receiptKey.current),
  });
  const receiptText = receipt.data
    ? [
        `OpenCade ${receipt.data.game_id} match: ${receipt.data.result}`,
        `Route: ${receipt.data.route}`,
        `Next-match invite: ${receipt.data.invite.code}`,
        `Expires: ${new Date(receipt.data.invite.expires_at).toLocaleString()}`,
      ].join("\n")
    : "";
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
          <li
            className={index <= active ? "active" : ""}
            aria-current={index === active ? "step" : undefined}
            key={step}
          >
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
      {coordinator.phase === "ready" && (
        <p className="status-copy" role="status">
          {preflightPending
            ? "Checking RetroArch, core, content, and TCP port…"
            : !snapshot?.compatibility_matched
              ? `Waiting for matching peer preflight (${snapshot?.preflight_count ?? 0}/2)`
              : snapshot.controller_ready_count < 2
                ? `Connect a controller · ready ${snapshot.controller_ready_count}/2`
                : launchBarrierPending || snapshot.barrier.ready_count < 2
                  ? `Compatibility matched · launch ready ${snapshot.barrier.ready_count}/2`
                  : launchSeconds && launchSeconds > 0
                    ? `Synchronized launch opens in ${launchSeconds}s`
                    : "Compatibility and synchronized launch barrier verified"}
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
      {leaveError && (
        <p className="form-error" role="alert">
          {leaveError}
        </p>
      )}
      {playableMatch.data && (
        <p className="status-copy" role="status">
          RetroArch netplay launched · PID {playableMatch.data.pid} · content{" "}
          {playableMatch.data.fingerprint.content_sha256.slice(0, 12)}
        </p>
      )}
      {receipt.data && (
        <article className="match-receipt" aria-labelledby="match-receipt-heading">
          <span className="sr-only" role="status">
            Verified match receipt created.
          </span>
          <p className="eyebrow">Verified match receipt</p>
          <h3 id="match-receipt-heading">Invite the next opponent</h3>
          <p>
            {receipt.data.game_id} · {receipt.data.route.replaceAll("_", " ")} · compatibility
            verified
          </p>
          <code>{receipt.data.invite.code}</code>
          <small>Expires {new Date(receipt.data.invite.expires_at).toLocaleString()}</small>
        </article>
      )}
      {receipt.isError && (
        <p className="form-error" id="receipt-help" role="alert">
          Both players’ verified reports must arrive before the next-match receipt can be created.
          Export or upload the missing report, then retry.
        </p>
      )}
      {evidenceUpload.isError && (
        <p className="form-error" id="evidence-help" role="alert">
          Match evidence could not be uploaded. Retry the upload before creating an invite.
        </p>
      )}
      {receiptCopied && (
        <p className="status-copy" role="status">
          Match receipt copied.
        </p>
      )}
      {receiptCopyError && (
        <p className="form-error" role="alert">
          {receiptCopyError}
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
        {room.isError && (
          <button className="secondary" onClick={() => void room.refetch()}>
            Retry room status
          </button>
        )}
        {(playableMatch.isError || coordinator.phase === "failed") && (
          <button
            className="secondary"
            onClick={() => {
              playableMatch.reset();
              resetCoordinator();
            }}
          >
            Retry match setup
          </button>
        )}
        {coordinator.phase === "ready" &&
          snapshot?.compatibility_matched &&
          snapshot.controller_ready_count < 2 && (
            <button className="secondary" onClick={retryPreflight}>
              Check controllers again
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
        {completedRoom && probeReport && !receipt.data && (
          <button
            className="primary"
            disabled={receipt.isPending || !evidenceUpload.isSuccess}
            aria-describedby={
              evidenceUpload.isError
                ? "evidence-help"
                : receipt.isError
                  ? "receipt-help"
                  : undefined
            }
            onClick={() => receipt.mutate()}
          >
            {evidenceUpload.isPending
              ? "Uploading match evidence…"
              : receipt.isPending
                ? "Creating next-match invite…"
                : "Create next-match invite"}
          </button>
        )}
        {evidenceUpload.isError && (
          <button
            className="secondary"
            onClick={() => {
              evidenceUpload.reset();
            }}
          >
            Retry evidence upload
          </button>
        )}
        {receipt.data && (
          <button
            className="primary"
            onClick={async () => {
              setReceiptCopyError("");
              try {
                if (!navigator.clipboard) throw new Error("Clipboard access is unavailable");
                await navigator.clipboard.writeText(receiptText);
                setReceiptCopied(true);
              } catch {
                setReceiptCopyError("Could not copy the receipt. Copy the invite code above.");
              }
            }}
          >
            Copy match receipt
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
            disabled={playableMatch.isPending || !canLaunch}
            onClick={() => playableMatch.mutate()}
          >
            {playableMatch.isPending ? "Launching RetroArch…" : "Launch playable alpha"}
          </button>
        )}
        <button
          className="secondary"
          disabled={isLeaving}
          onClick={() => {
            if (
              room.data?.state !== "finished" &&
              !window.confirm("Leave this match and stop its native session?")
            ) {
              return;
            }
            setIsLeaving(true);
            setLeaveError("");
            void (async () => {
              try {
                let stopError: unknown;
                if (playableMatch.data?.pid) {
                  try {
                    await stopGame(playableMatch.data.pid);
                  } catch (error) {
                    stopError = error;
                  }
                }
                if (
                  room.data &&
                  room.data.state !== "finished" &&
                  room.data.state !== "cancelled"
                ) {
                  await api.cancelRoom(token, roomId);
                }
                if (stopError) throw stopError;
                onDone();
              } catch (error) {
                setLeaveError(
                  error instanceof Error ? error.message : "Match could not be left safely"
                );
              } finally {
                setIsLeaving(false);
              }
            })();
          }}
        >
          {isLeaving
            ? "Leaving match…"
            : room.data?.state === "finished"
              ? "Return to games"
              : "Leave match"}
        </button>
      </div>
    </section>
  );
}
