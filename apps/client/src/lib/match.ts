import type {
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
  RoomPayload,
} from "@opencade/protocol";

export function parseMatchEndpoint(payload: unknown): MatchEndpointPayload | undefined {
  if (typeof payload !== "object" || payload === null) return undefined;
  const roomId = Reflect.get(payload, "room_id");
  const endpoint = Reflect.get(payload, "endpoint");
  const nonce = Reflect.get(payload, "nonce");
  if (typeof roomId !== "string" || typeof endpoint !== "string" || typeof nonce !== "string") {
    return undefined;
  }
  return { room_id: roomId, endpoint, nonce };
}

export function parseMatchCompletion(payload: unknown): MatchProbeCompletedPayload | undefined {
  if (typeof payload !== "object" || payload === null) return undefined;
  const roomId = Reflect.get(payload, "room_id");
  const framesReceived = Reflect.get(payload, "frames_received");
  const transcriptChecksum = Reflect.get(payload, "transcript_checksum");
  if (
    typeof roomId !== "string" ||
    typeof framesReceived !== "number" ||
    typeof transcriptChecksum !== "string"
  ) {
    return undefined;
  }
  return {
    room_id: roomId,
    frames_received: framesReceived,
    transcript_checksum: transcriptChecksum,
  };
}

export function matchParticipants(
  room: RoomPayload,
  localUserId: string
): { role: "host" | "guest"; peerUserId: string } | undefined {
  if (!room.guest_id) return undefined;
  if (room.host_id === localUserId) return { role: "host", peerUserId: room.guest_id };
  if (room.guest_id === localUserId) return { role: "guest", peerUserId: room.host_id };
  return undefined;
}
