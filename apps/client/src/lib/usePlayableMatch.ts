import { useEffect, useReducer, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
  RoomPayload,
} from "@opencade/protocol";
import { api } from "./api";
import { matchParticipants, selectNativeRoute } from "./match";
import { controllerPreflight } from "./controller";
import { initialMatchCoordinatorState, transitionMatchCoordinator } from "./matchCoordinator";
import { trackProductEvent } from "./telemetry";
import {
  launchRetroarchMatch,
  onEmulatorExit,
  retroarchPreflight,
  startNativeTcpTunnel,
  stopGame,
  stopNativeTcpTunnel,
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
  const preflightStarted = useRef(false);
  const readyStarted = useRef(false);
  const [coordinator, dispatch] = useReducer(
    transitionMatchCoordinator,
    initialMatchCoordinatorState
  );
  const participants = room ? matchParticipants(room, userId) : undefined;
  const snapshot = useQuery({
    queryKey: ["room-snapshot", roomId],
    queryFn: () => api.roomSnapshot(token, roomId),
    enabled: Boolean(room && participants),
    staleTime: 30_000,
  });
  const preflight = useMutation({
    mutationFn: async () => {
      if (!room) throw new Error("Room is unavailable for compatibility preflight");
      if (!participants) throw new Error("Two room participants are required for preflight");
      const local = await retroarchPreflight(room.game_id);
      return api.submitPreflight(token, roomId, {
        room_id: roomId,
        native_port_available: local.native_port_available,
        compatibility: {
          adapter: local.adapter,
          emulator_version: local.emulator_version ?? null,
          executable_sha256: local.executable_sha256,
          core_sha256: local.core_sha256,
          content_sha256: local.content_sha256,
        },
        controller: controllerPreflight(participants.role),
      });
    },
    onSuccess: (value) => queryClient.setQueryData(["room-snapshot", roomId], value),
    onError: (error) =>
      dispatch({
        type: "failed",
        error: error instanceof Error ? error.message : "Compatibility preflight failed",
      }),
  });
  const ready = useMutation({
    mutationFn: () => api.readyToLaunch(token, roomId),
    onSuccess: (value) => queryClient.setQueryData(["room-snapshot", roomId], value),
    onError: (error) =>
      dispatch({
        type: "failed",
        error: error instanceof Error ? error.message : "Launch barrier failed",
      }),
  });
  const launchAt = snapshot.data?.barrier.launch_at
    ? new Date(snapshot.data.barrier.launch_at).getTime()
    : undefined;
  const canLaunch = Boolean(
    snapshot.data?.compatibility_matched &&
    snapshot.data.barrier.ready_count === snapshot.data.barrier.required_count &&
    launchAt !== undefined &&
    launchAt <= Date.now()
  );
  const playableMatch = useMutation({
    mutationFn: async () => {
      if (!room || !participants || !localEndpoint || !peerEndpoint) {
        throw new Error("Peer session is incomplete");
      }
      if (probeReport?.transport !== "direct_udp") {
        throw new Error("Native gameplay requires a verified direct UDP path");
      }
      if (!canLaunch) throw new Error("Synchronized launch barrier is not ready");
      const nativeRoute = snapshot.data?.route ?? "direct_lan";
      const route =
        nativeRoute === "tcp_tunnel"
          ? { local: "127.0.0.1:55435", peer: "127.0.0.1:55435" }
          : selectNativeRoute(
              room,
              userId,
              localEndpoint.endpoint,
              peerEndpoint.endpoint,
              probeReport.transport,
              probeReport.candidate
            );
      dispatch({ type: "launch_requested" });
      await trackProductEvent(token, "launch_attempted", room.game_id).catch(() => false);
      if (nativeRoute === "tcp_tunnel") {
        const relay = await api.nativeTunnelTicket(token, roomId);
        if (relay.ticket.capability !== "native_tcp_tunnel") {
          throw new Error("Server returned an invalid native tunnel capability");
        }
        await startNativeTcpTunnel({
          relay_url: relay.relay_url,
          ticket: { ...relay.ticket, capability: "native_tcp_tunnel" },
          mode: participants.role === "host" ? "connect" : "listen",
          local_endpoint: "127.0.0.1:55435",
        });
      }
      let launch;
      try {
        const grant = await api.createLaunchGrant(token, roomId, route.local, route.peer);
        launch = await launchRetroarchMatch({ launch_grant: grant.grant });
      } catch (error) {
        if (nativeRoute === "tcp_tunnel") await stopNativeTcpTunnel(roomId).catch(() => undefined);
        throw error;
      }
      try {
        await api.startRoom(token, roomId);
      } catch (error) {
        await stopGame(launch.pid).catch(() => undefined);
        if (nativeRoute === "tcp_tunnel") await stopNativeTcpTunnel(roomId).catch(() => undefined);
        throw error;
      }
      await queryClient.invalidateQueries({ queryKey: ["room", roomId] });
      await trackProductEvent(token, "launch_succeeded", room.game_id).catch(() => false);
      return { ...launch, native_route: nativeRoute };
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
    if (coordinator.phase !== "ready" || preflightStarted.current) return;
    preflightStarted.current = true;
    preflight.mutate();
  }, [coordinator.phase, preflight]);

  useEffect(() => {
    if (
      !snapshot.data?.compatibility_matched ||
      snapshot.data.controller_ready_count !== 2 ||
      readyStarted.current
    )
      return;
    readyStarted.current = true;
    ready.mutate();
  }, [ready, snapshot.data?.compatibility_matched, snapshot.data?.controller_ready_count]);

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

  useEffect(
    () => () => {
      void stopNativeTcpTunnel(roomId);
    },
    [roomId]
  );

  return {
    coordinator,
    participants,
    playableMatch,
    snapshot: snapshot.data,
    preflightPending: preflight.isPending,
    launchBarrierPending: ready.isPending,
    canLaunch,
    retryPreflight: () => {
      preflightStarted.current = true;
      readyStarted.current = false;
      preflight.reset();
      preflight.mutate();
    },
    resetCoordinator: () => {
      preflightStarted.current = false;
      readyStarted.current = false;
      dispatch({ type: "reset" });
    },
  };
}
