import type { RoomPayload, RoomSnapshotPayload } from "@opencade/protocol";

const ROOM_STATES = new Set([
  "waiting",
  "ready",
  "challenging",
  "connecting",
  "playing",
  "finished",
  "cancelled",
]);
const ROUTES = new Set(["direct_lan", "tcp_tunnel"]);

function isBoundedCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= 255;
}

function isEvidenceCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= 4_294_967_295;
}

export function isRoomPayload(payload: unknown): payload is RoomPayload {
  return (
    typeof payload === "object" &&
    payload !== null &&
    typeof Reflect.get(payload, "id") === "string" &&
    typeof Reflect.get(payload, "game_id") === "string" &&
    typeof Reflect.get(payload, "host_id") === "string" &&
    (Reflect.get(payload, "guest_id") === null ||
      typeof Reflect.get(payload, "guest_id") === "string") &&
    ROOM_STATES.has(String(Reflect.get(payload, "state")))
  );
}

export function isRoomSnapshot(payload: unknown): payload is RoomSnapshotPayload {
  if (typeof payload !== "object" || payload === null) return false;
  const room = Reflect.get(payload, "room");
  const barrier = Reflect.get(payload, "barrier");
  const policy = Reflect.get(payload, "route_policy");
  const route = Reflect.get(payload, "route");
  return (
    isRoomPayload(room) &&
    Number.isSafeInteger(Reflect.get(payload, "revision")) &&
    Number(Reflect.get(payload, "revision")) >= 0 &&
    isBoundedCount(Reflect.get(payload, "preflight_count")) &&
    typeof Reflect.get(payload, "compatibility_matched") === "boolean" &&
    isBoundedCount(Reflect.get(payload, "controller_ready_count")) &&
    typeof barrier === "object" &&
    barrier !== null &&
    Reflect.get(barrier, "room_id") === room.id &&
    isBoundedCount(Reflect.get(barrier, "ready_count")) &&
    isBoundedCount(Reflect.get(barrier, "required_count")) &&
    (Reflect.get(barrier, "launch_at") === null ||
      (typeof Reflect.get(barrier, "launch_at") === "string" &&
        !Number.isNaN(Date.parse(String(Reflect.get(barrier, "launch_at")))))) &&
    ROUTES.has(String(route)) &&
    typeof policy === "object" &&
    policy !== null &&
    Reflect.get(policy, "route") === route &&
    typeof Reflect.get(policy, "reason") === "string" &&
    isEvidenceCount(Reflect.get(policy, "evidence_attempts")) &&
    isEvidenceCount(Reflect.get(policy, "evidence_verified")) &&
    Number(Reflect.get(policy, "evidence_verified")) <=
      Number(Reflect.get(policy, "evidence_attempts"))
  );
}

export function newestSnapshot(
  current: RoomSnapshotPayload | undefined,
  incoming: RoomSnapshotPayload
): RoomSnapshotPayload {
  return !current || incoming.revision >= current.revision ? incoming : current;
}
