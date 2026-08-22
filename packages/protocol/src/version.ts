export const PROTOCOL_VERSION = "1.0" as const;

export function isSupportedVersion(v: string): boolean {
  return v === "1.0" || v === "1";
}
