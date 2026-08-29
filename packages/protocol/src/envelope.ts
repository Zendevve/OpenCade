import { PROTOCOL_VERSION, isSupportedVersion } from "./version.js";

export type Envelope<T, Type extends string = string> = {
  type: Type;
  version: string;
  request_id: string;
  timestamp: string;
  payload: T;
};

function generateRequestId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  const s = "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx";
  return s.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export function createEnvelope<T, Type extends string>(
  type: Type,
  payload: T,
  opts?: Partial<Pick<Envelope<T, Type>, "version" | "request_id" | "timestamp">>
): Envelope<T, Type> {
  return {
    type,
    version: opts?.version ?? PROTOCOL_VERSION,
    request_id: opts?.request_id ?? generateRequestId(),
    timestamp: opts?.timestamp ?? new Date().toISOString(),
    payload,
  };
}

export function validateEnvelope<T>(
  envelope: Envelope<T>
): { ok: true } | { ok: false; error: string } {
  if (!isSupportedVersion(envelope.version)) {
    return { ok: false, error: `unsupported version: ${envelope.version}` };
  }
  if (typeof envelope.type !== "string" || envelope.type.trim().length === 0) {
    return { ok: false, error: "type must not be empty" };
  }
  if (typeof envelope.request_id !== "string" || envelope.request_id.length === 0) {
    return { ok: false, error: "request_id must not be empty" };
  }
  if (typeof envelope.timestamp !== "string" || Number.isNaN(Date.parse(envelope.timestamp))) {
    return { ok: false, error: "timestamp must be a valid ISO-8601 string" };
  }
  return { ok: true };
}

export function parseEnvelope<T = unknown>(raw: string): Envelope<T> {
  const decoded: unknown = JSON.parse(raw);
  if (typeof decoded !== "object" || decoded === null) {
    throw new Error("invalid envelope shape: missing required fields");
  }
  const parsed = decoded as Record<string, unknown>;
  const type = parsed.type;
  const version = parsed.version;
  const requestId = parsed.request_id;
  const timestamp = parsed.timestamp;
  if (
    typeof type !== "string" ||
    typeof version !== "string" ||
    typeof requestId !== "string" ||
    typeof timestamp !== "string" ||
    parsed.payload === undefined
  ) {
    throw new Error("invalid envelope shape: missing required fields");
  }
  // Validate inline on this hot path to avoid allocating validateEnvelope's result object.
  if (!isSupportedVersion(version)) {
    throw new Error(`unsupported version: ${version}`);
  }
  if (type.trim().length === 0) {
    throw new Error("type must not be empty");
  }
  if (requestId.length === 0) {
    throw new Error("request_id must not be empty");
  }
  if (Number.isNaN(Date.parse(timestamp))) {
    throw new Error("timestamp must be a valid ISO-8601 string");
  }
  return parsed as Envelope<T>;
}

export function serializeEnvelope<T>(envelope: Envelope<T>): string {
  return JSON.stringify(envelope);
}
