import { useEffect, useRef, useState } from "react";
import type {
  AlphaFailureStage,
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
  MatchReportTransport,
  RoomPayload,
} from "@opencade/protocol";
import { api } from "./api";
import { matchParticipants } from "./match";
import {
  cancelMatchProbe,
  reserveMatchProbe,
  runRelayMatchProbe,
  runReservedMatchProbe,
  type MatchEndpointCandidate,
  type MatchProbeReport,
} from "./native";
import type { OpenCadeSocket } from "./ws";

type LanMatchProbeOptions = {
  token: string;
  userId: string;
  roomId: string;
  room?: RoomPayload;
  socket: OpenCadeSocket | null;
  peerEndpoint?: MatchEndpointPayload;
  peerCompletion?: MatchProbeCompletedPayload;
  onRetry: () => void;
};

export type ProbeFailureEvidence = {
  stage: AlphaFailureStage;
  error_code: string;
  transport?: MatchReportTransport;
};

export function useLanMatchProbe({
  token,
  userId,
  roomId,
  room,
  socket,
  peerEndpoint,
  peerCompletion,
  onRetry,
}: LanMatchProbeOptions) {
  const [localEndpoint, setLocalEndpoint] = useState<MatchEndpointCandidate | null>(null);
  const [probeReport, setProbeReport] = useState<MatchProbeReport | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [probeFailure, setProbeFailure] = useState<ProbeFailureEvidence | null>(null);
  const [candidateRelayed, setCandidateRelayed] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const probeStarted = useRef(false);
  const resetStarted = useRef(false);
  const probeAttempt = useRef(0);
  const preparationGeneration = useRef(0);
  const reservationInFlight = useRef<Promise<MatchEndpointCandidate> | null>(null);

  useEffect(() => {
    if (!socket || !room || room.state !== "connecting" || localEndpoint) return;
    const attempt = probeAttempt.current;
    const preparation = ++preparationGeneration.current;
    let cancelled = false;
    const prepare = async () => {
      try {
        const reservation = reservationInFlight.current ?? reserveMatchProbe(roomId);
        if (!reservationInFlight.current) reservationInFlight.current = reservation;
        let candidate: MatchEndpointCandidate;
        try {
          candidate = await reservation;
        } finally {
          if (reservationInFlight.current === reservation) reservationInFlight.current = null;
        }
        if (cancelled || probeAttempt.current !== attempt) {
          if (preparationGeneration.current === preparation) await cancelMatchProbe(roomId);
          return;
        }
        setLocalEndpoint(candidate);
        const delivered = await relayUntilDelivered(
          socket,
          "match.endpoint",
          {
            room_id: roomId,
            endpoint: candidate.endpoint,
            reflexive_endpoint: candidate.reflexive_endpoint ?? null,
            nat: candidate.nat,
            nonce: candidate.nonce,
          },
          () => cancelled || probeAttempt.current !== attempt,
          (message) => {
            if (!cancelled && probeAttempt.current === attempt) setProbeError(message);
          }
        );
        if (delivered && !cancelled && probeAttempt.current === attempt) {
          setCandidateRelayed(true);
          setProbeError(null);
        }
      } catch (error) {
        if (!cancelled && probeAttempt.current === attempt) {
          setProbeError(errorMessage(error, "Failed to reserve LAN probe"));
          setProbeFailure({
            stage: "endpoint_reservation",
            error_code: "endpoint_reservation_failed",
          });
        }
      }
    };
    void prepare();
    return () => {
      cancelled = true;
    };
  }, [localEndpoint, room, roomId, socket]);

  useEffect(() => {
    if (
      !room ||
      !socket ||
      !localEndpoint ||
      !candidateRelayed ||
      !peerEndpoint ||
      peerEndpoint.room_id !== roomId ||
      probeStarted.current
    ) {
      return;
    }
    const participants = matchParticipants(room, userId);
    if (!participants) return;
    const attempt = probeAttempt.current;
    probeStarted.current = true;
    setProbeError(null);
    let cancelled = false;
    const run = async () => {
      try {
        let report: MatchProbeReport;
        try {
          report = await runReservedMatchProbe({
            room_id: roomId,
            game_id: room.game_id,
            local_user_id: userId,
            peer_user_id: participants.peerUserId,
            role: participants.role,
            peer_endpoint: peerEndpoint.endpoint,
            peer_reflexive_endpoint: peerEndpoint.reflexive_endpoint ?? undefined,
            peer_nonce: peerEndpoint.nonce,
          });
        } catch (directError) {
          if (!cancelled && probeAttempt.current === attempt) {
            setProbeError(
              `${errorMessage(directError, "Direct UDP failed")}; trying authenticated relay`
            );
          }
          let relay: Awaited<ReturnType<typeof api.relayTicket>>;
          try {
            relay = await api.relayTicket(token, roomId);
          } catch (error) {
            if (!cancelled && probeAttempt.current === attempt) {
              setProbeFailure({
                stage: "relay_ticket",
                error_code: "relay_ticket_failed",
                transport: "relay",
              });
            }
            throw error;
          }
          try {
            report = await runRelayMatchProbe({
              relay_url: relay.relay_url,
              ticket: relay.ticket,
              room_id: roomId,
              game_id: room.game_id,
              local_user_id: userId,
              peer_user_id: participants.peerUserId,
              role: participants.role,
              local_nonce: localEndpoint.nonce,
              peer_nonce: peerEndpoint.nonce,
            });
          } catch (error) {
            if (!cancelled && probeAttempt.current === attempt) {
              setProbeFailure({
                stage: "relay",
                error_code: "relay_probe_failed",
                transport: "relay",
              });
            }
            throw error;
          }
        }
        if (cancelled || probeAttempt.current !== attempt) return;
        setProbeFailure(null);
        setProbeError(null);
        setProbeReport(report);
        await relayUntilDelivered(
          socket,
          "match.probe.completed",
          {
            room_id: roomId,
            frames_received: report.frames_received,
            transcript_checksum: report.transcript_checksum,
          },
          () => cancelled || probeAttempt.current !== attempt,
          (message) => {
            if (!cancelled && probeAttempt.current === attempt) setProbeError(message);
          }
        );
      } catch (error) {
        if (!cancelled && probeAttempt.current === attempt) {
          setProbeError(errorMessage(error, "LAN match probe failed"));
        }
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, [candidateRelayed, localEndpoint, peerEndpoint, room, roomId, socket, token, userId]);

  const retry = async () => {
    if (resetStarted.current) return;
    resetStarted.current = true;
    probeAttempt.current += 1;
    setIsResetting(true);
    try {
      await reservationInFlight.current?.catch(() => undefined);
      await cancelMatchProbe(roomId);
      probeStarted.current = false;
      setProbeError(null);
      setProbeFailure(null);
      setProbeReport(null);
      setCandidateRelayed(false);
      setLocalEndpoint(null);
      onRetry();
    } catch (error) {
      setProbeError(errorMessage(error, "Could not reset the LAN probe; try again"));
    } finally {
      resetStarted.current = false;
      setIsResetting(false);
    }
  };

  return { localEndpoint, probeReport, probeError, probeFailure, isResetting, retry };
}

async function relayUntilDelivered(
  socket: OpenCadeSocket,
  type: string,
  payload: unknown,
  cancelled: () => boolean,
  onWaiting: (message: string | null) => void
): Promise<boolean> {
  while (!cancelled()) {
    try {
      await socket.send(type, payload);
      onWaiting(null);
      return true;
    } catch (error) {
      onWaiting(`${errorMessage(error, "Peer unavailable")}; waiting for peer connection`);
      await new Promise((resolve) => window.setTimeout(resolve, 1_000));
    }
  }
  return false;
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}
