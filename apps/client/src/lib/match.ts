import type {
  MatchEndpointPayload,
  MatchProbeCompletedPayload,
  RoomPayload,
} from "@opencade/protocol";

export function parseMatchEndpoint(payload: unknown): MatchEndpointPayload | undefined {
  if (typeof payload !== "object" || payload === null) return undefined;
  const roomId = Reflect.get(payload, "room_id");
  const endpoint = Reflect.get(payload, "endpoint");
  const reflexiveEndpoint = Reflect.get(payload, "reflexive_endpoint");
  const nat = Reflect.get(payload, "nat");
  const nonce = Reflect.get(payload, "nonce");
  if (
    typeof roomId !== "string" ||
    typeof endpoint !== "string" ||
    (reflexiveEndpoint !== undefined &&
      reflexiveEndpoint !== null &&
      typeof reflexiveEndpoint !== "string") ||
    (nat !== undefined && nat !== "unknown" && nat !== "open" && nat !== "mapped") ||
    typeof nonce !== "string"
  ) {
    return undefined;
  }
  return {
    room_id: roomId,
    endpoint,
    reflexive_endpoint: reflexiveEndpoint ?? null,
    nat: nat ?? "unknown",
    nonce,
  };
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

export function nativeLanEndpoint(endpoint: string, port = 55_435): string {
  const parsed = new URL(`udp://${endpoint}`);
  if (!parsed.hostname || !Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error("Invalid native LAN endpoint");
  }
  const host = parsed.hostname.startsWith("[")
    ? parsed.hostname
    : parsed.hostname.includes(":")
      ? `[${parsed.hostname}]`
      : parsed.hostname;
  return `${host}:${port}`;
}
