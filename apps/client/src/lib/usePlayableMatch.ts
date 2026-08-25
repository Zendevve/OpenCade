import { useEffect, useReducer } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type {
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
  RoomPayload,
} from "@opencade/protocol";
import { api, getApiBase } from "./api";
import { matchParticipants, nativeLanEndpoint } from "./match";
import { initialMatchCoordinatorState, transitionMatchCoordinator } from "./matchCoordinator";
import {
  launchRetroarchMatch,
  onEmulatorExit,
  stopGame,
  type MatchEndpointCandidate,
  type MatchProbeReport,
} from "./native";

type PlayableMatchOptions = {
  token: string;
  userId: string;
  roomId: string;
  room?: RoomPayload;
  localEndpoint: MatchEndpointCandidate | null;
  peerEndpoint?: MatchEndpointPayload;
  probeReport: MatchProbeReport | null;
  peerCompletion?: MatchProbeCompletedPayload;
};

export function usePlayableMatch({
  token,
  userId,
  roomId,
  room,
  localEndpoint,
  peerEndpoint,
  probeReport,
  peerCompletion,
}: PlayableMatchOptions) {
  const queryClient = useQueryClient();
  const [coordinator, dispatch] = useReducer(
    transitionMatchCoordinator,
    initialMatchCoordinatorState
  );
  const participants = room ? matchParticipants(room, userId) : undefined;
  const playableMatch = useMutation({
    mutationFn: async () => {
      if (!room || !participants || !localEndpoint || !peerEndpoint) {
        throw new Error("Peer session is incomplete");
      }
      if (probeReport?.transport !== "direct_udp") {
        throw new Error("Native gameplay requires a verified direct UDP path");
      }
      dispatch({ type: "launch_requested" });
      const grant = await api.createLaunchGrant(
        token,
        roomId,
        nativeLanEndpoint(localEndpoint.endpoint),
        nativeLanEndpoint(peerEndpoint.endpoint)
      );
      const launch = await launchRetroarchMatch({
        api_url: getApiBase(),
        session_token: token,
        launch_grant: grant.grant,
      });
      try {
        await api.startRoom(token, roomId);
      } catch (error) {
        await stopGame(launch.pid).catch(() => undefined);
        throw error;
      }
      await queryClient.invalidateQueries({ queryKey: ["room", roomId] });
      return launch;
    },
    onSuccess: () => dispatch({ type: "native_spawned" }),
    onError: (error) =>
      dispatch({
        type: "failed",
        error: error instanceof Error ? error.message : "Native launch failed",
      }),
  });

  useEffect(() => {
    if (!probeReport) return;
    dispatch({
      type: "probe_verified",
      transport: probeReport.transport,
      candidate: probeReport.candidate,
    });
  }, [probeReport]);

  useEffect(() => {
    if (!probeReport || !peerCompletion) return;
    if (
      peerCompletion.frames_received !== probeReport.frames_received ||
      peerCompletion.transcript_checksum !== probeReport.transcript_checksum
    ) {
      dispatch({ type: "failed", error: "Peer transcript does not match the local LAN probe" });
      return;
    }
    dispatch({ type: "peer_transcript_verified" });
  }, [peerCompletion, probeReport]);

  useEffect(() => {
    if (room?.state === "playing") dispatch({ type: "room_playing" });
    if (room?.state === "finished") dispatch({ type: "room_finished" });
  }, [room?.state]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onEmulatorExit((event) => {
      if (event.room_id !== roomId || cancelled) return;
      dispatch({ type: "native_exited" });
      void queryClient.invalidateQueries({ queryKey: ["room", roomId] });
    }).then((cleanup) => {
      if (cancelled) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient, roomId]);

  return {
    coordinator,
    participants,
    playableMatch,
    resetCoordinator: () => dispatch({ type: "reset" }),
  };
}
