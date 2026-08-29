export type GameDefinition = {
  schema_version: 1;
  id: string;
  name: string;
  emulator: "fbneo" | "flycast" | "snes9x" | "retroarch_test";
  launch: { args: string[] };
  validation?: { required_files?: string[]; bios?: string };
  metadata?: { year?: number; developer?: string; players?: number };
};

export function validateGameDefinition(def: GameDefinition): string | null {
  if (def.schema_version !== 1) return `unsupported schema_version ${def.schema_version}`;
  if (!/^[a-z0-9_-]{3,20}$/.test(def.id)) return `id '${def.id}' must match ^[a-z0-9_-]{3,20}$`;
  if (!def.name || def.name.trim().length === 0) return "name must not be empty";
  const allowed = ["fbneo", "flycast", "snes9x", "retroarch_test"] as const;
  if (!(allowed as readonly string[]).includes(def.emulator))
    return `emulator '${def.emulator}' must be one of ${allowed.join(", ")}`;
  if (!def.launch || !Array.isArray(def.launch.args) || def.launch.args.length === 0)
    return "launch.args must not be empty";
  if (!def.launch.args.some((a) => a.includes("{rom}")))
    return "launch.args must contain {rom} placeholder";
  return null;
}

export function renderArgs(def: GameDefinition, romPath: string): string[] {
  return def.launch.args.map((a) => a.replaceAll("{rom}", romPath));
}

export const GAME_IDS = ["kof98", "sfiii3", "garou", "kof2002", "mvc2", "opencade_test"] as const;
export type GameId = (typeof GAME_IDS)[number];
