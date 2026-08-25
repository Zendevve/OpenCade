import type { RoomSnapshotPayload } from "@opencade/protocol";

export function newestSnapshot(
  current: RoomSnapshotPayload | undefined,
  incoming: RoomSnapshotPayload
): RoomSnapshotPayload {
  return !current || incoming.revision >= current.revision ? incoming : current;
}
