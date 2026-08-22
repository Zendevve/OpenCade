import { describe, it, expect } from "vitest";
import { validateGameDefinition, renderArgs } from "./index.js";
import type { GameDefinition } from "./index.js";

function makeDef(overrides: Partial<GameDefinition> = {}): GameDefinition {
  return {
    schema_version: 1,
    id: "kof98",
    name: "The King of Fighters '98",
    emulator: "fbneo",
    launch: { args: ["-rom", "{rom}", "-window"] },
    validation: { required_files: ["kof98.zip", "neogeo.zip"], bios: "neogeo.zip" },
    metadata: { year: 1998, developer: "SNK", players: 2 },
    ...overrides,
  };
}

describe("validateGameDefinition", () => {
  it("accepts valid kof98", () => {
    expect(validateGameDefinition(makeDef())).toBeNull();
  });

  it("rejects bad schema_version", () => {
    expect(validateGameDefinition(makeDef({ schema_version: 2 as 1 }))).not.toBeNull();
  });

  it("rejects bad id", () => {
    expect(validateGameDefinition(makeDef({ id: "Bad!" }))).toMatch(/id/);
  });

  it("rejects missing {rom}", () => {
    expect(validateGameDefinition(makeDef({ launch: { args: ["-window"] } }))).toMatch(/\{rom\}/);
  });

  it("rejects unknown emulator", () => {
    expect(validateGameDefinition(makeDef({ emulator: "unknown" as "fbneo" }))).toMatch(/emulator/);
  });
});

describe("renderArgs", () => {
  it("substitutes {rom} everywhere", () => {
    const def = makeDef({ launch: { args: ["-rom", "{rom}", "-bios", "{rom}.bios"] } });
    expect(renderArgs(def, "/roms/kof98.zip")).toEqual(["-rom", "/roms/kof98.zip", "-bios", "/roms/kof98.zip.bios"]);
  });

  it("produces full launch line for sfiii3", () => {
    const def = makeDef({ id: "sfiii3", launch: { args: ["-rom", "{rom}", "-window"] } });
    expect(renderArgs(def, "C:/ROMS/sfiii3.zip")).toEqual(["-rom", "C:/ROMS/sfiii3.zip", "-window"]);
  });
});
