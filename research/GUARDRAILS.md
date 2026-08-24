# OpenCade — Clean-Room Guardrails

> **Read this before touching `D:/Fightcade` or writing any protocol, networking, or emulator code.**
>
> PRD references: **§32 Reverse-Engineering Research Workspace** and **§33 Clean-Room Rule**.
> This file operationalises those sections. Violations will be reverted and may result in a ban.

---

## 1. Purpose

OpenCade is an **independent, open-source reimplementation** inspired by Fightcade, not a copy of it. The existing Fightcade installation at `D:/Fightcade` is a **research / reference environment** to be treated as a black box. We document what we observe and then write original code from those observations plus public specifications.

The guardrails ensure:

- No proprietary source, binaries, ROMs, or assets are committed or redistributed.
- Every non-trivial behaviour has an auditable `Observation → Documentation → Design → Implementation` trail.
- The repository contains only original work, permissively licensed dependencies, and public specs.

---

## 2. The Clean-Room Pipeline (PRD §33)

For **every** potentially proprietary behaviour — networking, signaling, NAT traversal, lobby presence, challenge flow, emulator launch — you MUST follow the four steps in order:

```
Observation          Documentation         Design               Implementation
─────────────  →  ────────────────  →  ─────────────  →  ────────────────
Black-box use      Written note       Original design     Original code
pcaps, logs,       evidence +         in docs/ or PR      in apps/packages/
screenshots,       confidence +       citing only the     citing the design,
timing             implication        note + public specs note — never
                                                        proprietary source
```

### Step 1 — Observation

- Interact with `D:/Fightcade` without decompiling or patching it.
- Capture evidence: packet captures (`.pcap`/`.pcapng`), runtime logs, screenshots, UI recordings, timing measurements.
- Store raw evidence under `research/` only:

  ```
  research/
  ├── observations/   # dated markdown notes, screenshots
  ├── protocol/       # capture summaries, message tables
  ├── network/        # pcaps, NAT test results
  ├── behavior/       # UI / flow descriptions
  ├── binaries/       # local copies of Fightcade binaries for analysis (GITIGNORED)
  └── notes/          # synthesised findings with confidence + implication
  ```

  `research/binaries/` is **gitignored by design** — it must never be committed.

### Step 2 — Documentation

Write a note in `research/notes/` or `research/behavior/` using this template:

```markdown
# Observation: <short title>

Date: YYYY-MM-DD
Observer: <name>
Source: D:/Fightcade — <version, e.g. 2.1.45> / public spec

## Observation

<One paragraph: what happened, in your own words.>

## Evidence

- Packet capture: research/network/2026-08-22-lobby-join.pcapng
- Log excerpt: research/observations/2026-08-22-auth-flow.md:12-34
- Screenshot: research/observations/2026-08-22-lobby.png

## Confidence

High | Medium | Low — and why.

## Implementation Implication

<What OpenCade needs to do, without copying proprietary details.>
Example: "OpenCade requires a persistent WebSocket after authentication
for presence/chat/signaling; the exact frame format is TBD via public design."

## References

- Public spec: RFC 5389 §...
- GGPO open-source docs: ...
```

- Be precise about evidence; do not quote proprietary source.
- State confidence honestly. Low-confidence observations must not drive protocol decisions.

### Step 3 — Design

- In `docs/` (ADR) or the PR description, propose an OpenCade design that cites **only** your observation note and public specs.
- Do not reference proprietary file paths, function names, or decompiled output.
- Get maintainer feedback before coding non-trivial protocol changes.

### Step 4 — Implementation

- Write original Rust / TypeScript from the design.
- Do not have `D:/Fightcade` binaries, `lib/main.js`, or decompiler output open while coding the same feature.
- If you authored the observation note and the implementation for the same feature, leave a time gap and disclose it in the PR.

### Separation of Duties

For protocol / networking work the **ideal** is two different contributors: one documents the observation, another implements from the document. When one person does both, the PR must state that and link the intermediate note.

---

## 3. What You MUST NOT Commit — Hard Blocklist

The following must **never** appear in any commit, PR, issue attachment, or release artefact. CI includes a blocklist scan and review will reject them.

| Category                    | Examples                                                                                                                                       | Why                                      |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| **Proprietary executables** | `fcade.exe`, `fcadefbneo.exe`, `frm.exe`, `Fightcade*.exe`, `fcade-upd.exe`, `fc2-electron.exe`                                                | Fightcade binaries are proprietary       |
| **Proprietary libraries**   | `ggponet.dll`, `ggpo*.dll`, `kailleraclient.dll`, `kaillera*.dll`, any `*.dll` from `D:/Fightcade/emulator/`                                   | Proprietary netcode libraries            |
| **Electron wrapper source** | `fc2-electron/resources/app/lib/main.js`, `fc2-electron/resources/app/lib/static/login.js`, any `*.asar` unpack                                | Proprietary client source                |
| **ROMs / disk images**      | `*.zip`, `*.7z`, `*.chd`, `*.iso`, `*.cue`, `*.gdi` containing game data; `emulator/fbneo_roms.json` / `snes9x_roms.json` etc. copied verbatim | Copyrighted game content                 |
| **Proprietary assets**      | `assets/*.wav` (e.g. `kof98-challenge.wav`, `sfiii3-challenge.wav`), `assets/*.ico`, `assets/icon-128.png` from Fightcade                      | Proprietary media                        |
| **Credentials / secrets**   | API keys, tokens, passwords, `.env` with real values, `%APPDATA%/OpenCade/` dumps, WebSocket auth headers with secrets                        | Security                                 |
| **Decompiled output**       | Any file derived from decompiling / transcribing `fcade.exe`, `ggponet.dll`, or `lib/main.js`                                                  | Clean-room violation                     |
| **Research binaries**       | Anything under `research/binaries/`                                                                                                            | Must stay local; directory is gitignored |

Additional blocked patterns enforced by `.gitignore` and CI:

```
research/binaries/**
emulator/
ROMs/  roms/  rom/
*.zip  *.chd  *.iso
assets/*.wav  *.wav
fcade*.exe  ggponet.dll  kailleraclient.dll
%APPDATA%/OpenCade/
.env  *.pem  *.key  *.log
```

If you need to reference a filename for documentation (e.g. "the file `ggponet.dll` was observed in `emulator/fbneo/`"), mentioning the _name_ in prose is allowed. Committing the _file_ is not.

### Specifically Forbidden — Do Not Even Locally Stage

These exact files from `D:/Fightcade` (non-exhaustive):

- `emulator/fcade.exe`
- `emulator/fcadefbneo.exe`
- `emulator/*/ggponet.dll` and `emulator/*/kailleraclient.dll`
- `emulator/*.json` committed as game definitions without transformation and review (see §5)
- `fc2-electron/resources/app/lib/main.js`
- `fc2-electron/resources/app/lib/static/login.js`
- `assets/*-challenge.wav` (all 20+ challenge sounds)
- Any `ROMs/*.zip` or `emulator/**/*.zip`

---

## 4. What IS Allowed

| Category                                           | Allowed                                                                                                                      | Conditions                                                                                                                                                   |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Original code**                                  | All Rust / TypeScript you write from scratch for OpenCade                                                                   | Must be your original work or properly attributed; licensed Apache-2.0                                                                                       |
| **Permissively licensed deps**                     | Crates / npm packages under Apache-2.0, MIT, BSD, ISC                                                                        | Must be declared in `Cargo.toml` / `package.json`; `cargo deny` / `pnpm audit` clean                                                                         |
| **Public specifications**                          | GGPO rollback concepts (public docs), STUN **RFC 5389**, TURN **RFC 5766**, WebSocket **RFC 6455**, HTTP, etc.               | Cite the RFC / public doc; do not cite proprietary source as authority                                                                                       |
| **FBNeo training-mode Lua — shape reference only** | The _structure_ of open-source Lua training-mode scripts (e.g. function names, menu shape) as a reference for adapter design | **Shape only, under its own license.** Do not copy implementation. Link to the upstream repo and note the license. Prefer re-deriving from public FBNeo docs |
| **Observation notes**                              | Your own `research/notes/*.md`, `research/observations/*.md`, `research/protocol/*.md`                                       | Must follow the template in §2; no proprietary source pasted                                                                                                 |
| **Game definitions you author**                    | Hand-written `packages/game-definitions/games/*.toml` per the schema in PRD §17                                              | Declarative, validated; `id`, `name`, `emulator`, `[launch]`, `[validation]`                                                                                 |
| **Local importer output (with review)**            | TOML generated locally by the importer that converts `D:/Fightcade/emulator/*.json`                                          | The importer itself may be committed; its **output TOML must not be committed without maintainer review** (see §5)                                           |

### Public Specs — Preferred References

- STUN: **RFC 5389** — use for NAT traversal design, not Fightcade's capture.
- TURN: **RFC 5766** / **RFC 8656** — for relay fallback.
- GGPO: Use the open-source `ggpo` / `rollback` literature and public articles; do not reverse the proprietary `ggponet.dll`.
- WebSocket / HTTPS: RFC 6455, Fetch, etc. for signaling transport.

When a behaviour can be explained by a public spec, cite the spec as the source of truth and treat the Fightcade observation as corroboration only.

---

## 5. The `emulator/*.json` → TOML Importer Rule

`D:/Fightcade/emulator/` contains large manifests such as `fbneo_roms.json` (727 KB), `snes9x_roms.json`, `fbneo_sms_roms.json`, etc. These are **reference data** derived from the proprietary distribution.

**Rule:**

- You MAY write and commit an **importer tool** (e.g. `packages/game-definitions/importer/` or `tools/import-fbneo/`) that **locally** reads `D:/Fightcade/emulator/*.json` and emits declarative TOML per PRD §17. The importer must be original code.
- You MUST NOT commit the importer's output TOML en masse without review. Generated files are **local-only** until a maintainer has reviewed a _small, representative sample_ for licensing, accuracy, and schema compliance.
- The importer must not embed or copy large JSON fragments as literals in the repository. It reads them at runtime from the user's local `D:/Fightcade` path, which is gitignored.
- If sample TOML is approved, it should be hand-curated and attributed as "derived from local observation of emulator manifests; independently authored TOML" — not a verbatim JSON-to-TOML dump.

Suggested importer invocation (local only):

```powershell
# From D:/OpenCade
cargo run -p game-definitions-importer -- --source "D:/Fightcade/emulator" --out "packages/game-definitions/games" --dry-run
# Review the diff, then run without --dry-run for approved entries only
```

Add the importer's output directory to `.gitignore` if you generate in bulk:

```gitignore
# local importer output — review before committing any file
packages/game-definitions/games/*.generated.toml
```

---

## 6. Research Workspace Hygiene

```
research/
├── observations/   # raw notes — may contain screenshots, log excerpts
├── protocol/       # message tables, sequence diagrams from captures
├── network/        # pcaps (gitignored: *.pcap, *.pcapng)
├── behavior/       # UI flow notes
├── binaries/       # Fightcade binaries for local analysis — GITIGNORED, never pushed
└── notes/          # synthesised, high-value findings (the only research dir that should have many commits)
```

- `research/binaries/` — stays empty in git (`!research/binaries/.gitkeep` only). Populate locally by copying from `D:/Fightcade` if needed; never `git add` it.
- `research/network/*.pcap*` — gitignored; summarise findings in markdown instead.
- `research/` as a whole is **not shipped** with the Tauri client or Docker images. `Dockerfile` and `tauri.conf.json` must exclude it.

Pre-commit hook (recommended) — add to `.git/hooks/pre-commit`:

```bash
#!/bin/sh
if git diff --cached --name-only | grep -Eq 'research/binaries/|fcade.*\.exe|ggponet\.dll|kaillera.*\.dll|assets/.*-challenge\.wav|\.zip$'; then
  echo "Blocked: commit contains blocklisted proprietary files. See research/GUARDRAILS.md"
  exit 1
fi
```

---

## 7. Dependency Hygiene (Apache-2.0)

- All first-party OpenCade code is **Apache-2.0**.
- New dependencies must be **Apache-2.0, MIT, BSD, or ISC** compatible. Avoid GPL / AGPL / SSPL unless explicitly approved with a compatibility note.
- Run before each PR that adds a dependency:

  ```powershell
  cargo deny check licenses bans sources
  pnpm audit --audit-level=moderate
  cargo tree | findstr "GPL"  # quick sanity check on Windows
  ```

- Vendor nothing proprietary. If you vendor an Apache-2.0 dependency, preserve its `LICENSE` and `NOTICE`.

---

## 8. CI Enforcement

CI will fail a PR if it detects:

- Blocklisted filenames or extensions (`*.exe`, `ggponet.dll`, `*.zip` ROMs, `*.wav` challenge sounds, `lib/main.js`).
- `research/binaries/` content.
- `cargo fmt -- --check` or `cargo clippy -- -D warnings` failures.
- `pnpm format:check` / `pnpm lint` / `pnpm typecheck` failures.
- Unapproved licenses via `cargo deny`.

Maintainers will also manually verify the clean-room trail for protocol / networking PRs.

---

## 9. If You Are Unsure

Ask before you commit. Open a `research/` discussion issue or ping a maintainer. It is always cheaper to ask than to revert a tainted commit.

**When in doubt: document the observation, do not copy the artefact.**

---

_Last updated: 2026-08-22 — PRD v MVP Specification (Phase 0). Keep this file in sync with `CONTRIBUTING.md` and `.gitignore`._
