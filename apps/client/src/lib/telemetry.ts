import type { ProductEventName, ProductEventPayload, ReadinessCheckId } from "@opencade/protocol";
import { api } from "./api";

export const TELEMETRY_CONSENT_KEY = "opencade.product-telemetry-consent";
const TELEMETRY_SESSION_KEY = "opencade.product-telemetry-session";

type StorageAccess = Pick<Storage, "getItem" | "setItem">;

export function getTelemetryConsent(storage?: StorageAccess): boolean | null {
  const target = storage ?? browserStorage("local");
  if (!target) return null;
  try {
    const value = target.getItem(TELEMETRY_CONSENT_KEY);
    return value === "granted" ? true : value === "declined" ? false : null;
  } catch {
    return null;
  }
}

export function setTelemetryConsent(consent: boolean, storage?: StorageAccess): boolean {
  const target = storage ?? browserStorage("local");
  if (!target) return false;
  try {
    target.setItem(TELEMETRY_CONSENT_KEY, consent ? "granted" : "declined");
    return true;
  } catch {
    return false;
  }
}

export function createProductEvent(
  anonymousSessionId: string,
  event: ProductEventName,
  gameId: string,
  blockedChecks: ReadinessCheckId[] = [],
  eventId = crypto.randomUUID()
): ProductEventPayload {
  return {
    event_id: eventId,
    anonymous_session_id: anonymousSessionId,
    event,
    game_id: gameId,
    blocked_checks: blockedChecks,
  };
}

export async function trackProductEvent(
  token: string,
  event: ProductEventName,
  gameId: string,
  blockedChecks: ReadinessCheckId[] = []
): Promise<boolean> {
  if (getTelemetryConsent() !== true) return false;
  const anonymousSessionId = telemetrySessionId();
  if (!anonymousSessionId) return false;
  await api.recordProductEvent(
    token,
    createProductEvent(anonymousSessionId, event, gameId, blockedChecks)
  );
  return true;
}

function telemetrySessionId(): string | null {
  const storage = browserStorage("session");
  if (!storage) return null;
  try {
    const existing = storage.getItem(TELEMETRY_SESSION_KEY);
    if (existing) return existing;
    const created = crypto.randomUUID();
    storage.setItem(TELEMETRY_SESSION_KEY, created);
    return created;
  } catch {
    return null;
  }
}

function browserStorage(kind: "local" | "session"): StorageAccess | null {
  if (typeof window === "undefined") return null;
  return kind === "local" ? window.localStorage : window.sessionStorage;
}
