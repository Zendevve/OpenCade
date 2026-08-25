# Contributing to OpenCade

Thank you for helping build an open, self-hostable alternative for arcade netplay. This document describes how to contribute safely and consistently. By contributing you agree that your contributions will be licensed under the **Apache License 2.0** (see `LICENSE`).

> **Golden rule:** OpenCade is a **clean-room reimplementation**. We study the existing proprietary platform only as a black box, write down what we observe, and then independently design and implement. We never copy, paste, decompile, or redistribute proprietary code, binaries, ROMs, or assets.

---

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Ways to Contribute](#ways-to-contribute)
3. [Prerequisites](#prerequisites)
4. [Repository Layout](#repository-layout)
5. [Branch Naming](#branch-naming)
6. [Commit Style](#commit-style)
7. [Clean-Room Rule — Mandatory](#clean-room-rule--mandatory)
8. [What You Must Never Commit](#what-you-must-never-commit)
9. [Code Style & Quality Gates](#code-style--quality-gates)
10. [Testing Before a PR](#testing-before-a-pr)
11. [Pull Request Process](#pull-request-process)
12. [Reporting Issues & Security](#reporting-issues--security)

---

## Support the Project

OpenCade is Apache-2.0 and self-hosted by design. If you can't contribute code right now, you can still keep the hard systems work moving — NAT traversal, relay fallback, and the FBNeo netplay seam.

**[☕ Support via Buy Me a Coffee — https://buymeacoffee.com/zendevve](https://buymeacoffee.com/zendevve)** — every coffee funds LAN test rigs, CI, and docs. No paywall, no premium: just open, auditable netplay.

---

## Code of Conduct

Be respectful, assume good intent, and keep discussion technical. Harassment, discrimination, or distribution of pirated content will result in a ban. See `CODE_OF_CONDUCT.md` if present.

## Ways to Contribute

- **Code** — Rust (server, relay, Tauri native layer), TypeScript/React (client), adapters, docs.
- **Documentation** — Install guides, protocol notes, game definitions, diagnostics.
- **Testing** — Unit / integration / networking / end-to-end (see PRD §37).
- **Bug reports & feature requests** — Use the issue templates in `.github/ISSUE_TEMPLATE/`.
- **Self-hosting feedback** — Docker Compose, PostgreSQL migrations, relay operation.

If you are unsure where to start, look for issues labelled `good first issue` or `help wanted`.

## Prerequisites

| Tool       | Minimum version    | Notes                                                             |
| ---------- | ------------------ | ----------------------------------------------------------------- |
| Rust       | stable (rustup)    | `cargo fmt`, `cargo clippy` required                              |
| Node.js    | 20 LTS             | `pnpm` is the package manager                                     |
| pnpm       | 9.x                | `corepack enable` or `npm i -g pnpm`                              |
| PostgreSQL | 16                 | only for server work; or use `docker compose up -d postgres`      |
| Tauri deps | WebView2 (Win 10+) | See [Tauri prerequisites](https://tauri.app/start/prerequisites/) |

```powershell
# one-time setup
corepack enable
pnpm install
cargo --version
pnpm --version
```

Copy the example environment file and never commit the real one:

```powershell
Copy-Item .env.example .env
# edit .env locally — it is gitignored
```

## Repository Layout

```
opencade/
├── apps/
│   ├── client/            # Tauri + React + TypeScript desktop client
│   │   └── src-tauri/     # Rust native layer (process/fs/logging)
│   └── server/            # Rust + Axum + PostgreSQL backend
├── packages/
│   ├── protocol/          # Versioned signaling / WebSocket types
│   ├── game-definitions/  # Declarative TOML game definitions
│   ├── emulator-sdk/      # Adapter trait + launch/validate helpers
│   ├── networking/        # Signaling, NAT traversal, relay client
│   └── shared/            # Shared TS/Rust types
├── adapters/
│   ├── fbneo/             # FBNeo adapter (first adapter)
│   └── mame/              # (future)
├── services/
│   └── relay/             # authenticated readiness-probe WebSocket fallback
├── research/              # NOT SHIPPED — observations only (see GUARDRAILS.md)
├── docs/
├── docker/
├── tests/
└── research/GUARDRAILS.md # clean-room rules (read this first)
```

## Branch Naming

Create branches from `main`. Use the prefix + short kebab-case slug:

```
<type>/<scope>-<short-description>
```

| Prefix      | Use                                                      |
| ----------- | -------------------------------------------------------- |
| `feat/`     | New feature                                              |
| `fix/`      | Bug fix                                                  |
| `docs/`     | Documentation only                                       |
| `chore/`    | Tooling, CI, deps, chores                                |
| `refactor/` | Behaviour-preserving restructure                         |
| `research/` | Research-only notes (never ships to `apps/`/`packages/`) |

Examples:

```
feat/lobby-presence
fix/relay-reconnect-backoff
docs/clean-room-examples
chore/ci-clippy-strict
research/nat-hole-punch-observations
```

Keep branch names ≤ 50 chars, lower-case, no spaces. Personal forks may use `yourname/feat/...` but the same prefix rule applies.

## Commit Style

We use **Conventional Commits**. This powers changelogs and `cargo`/`pnpm` release tooling.

```
<type>(<scope>): <imperative summary>

[optional body — why, not what]

[optional footer: Closes #123, Breaking change: ...]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.

Scopes (examples): `client`, `server`, `protocol`, `relay`, `emulator-sdk`, `fbneo`, `game-defs`, `docs`, `ci`.

Rules:

- Use imperative mood: `add`, `fix`, `remove` — not `added` / `fixes`.
- Summary ≤ 72 chars, no trailing period.
- One logical change per commit. Squash fixups before requesting review.
- Reference issues in the footer (`Closes #123`).
- Never commit secrets, tokens, or `research/binaries/`.

Examples:

```
feat(protocol): version signaling envelope with request_id

fix(fbneo): escape ROM path before spawning process

docs(research): document persistent TCP observation with pcap evidence

chore(ci): enforce cargo fmt and clippy -D warnings
```

We may squash-merge. Keep history readable.

## Clean-Room Rule — Mandatory

For **every** behaviour that could be proprietary, follow the four-step pipeline (PRD §32–33):

```
Observation → Documentation → Design → Implementation
```

1. **Observation** — Interact with the proprietary platform as a black box. Record what you see: packet captures, UI screenshots, log excerpts, timing. Put raw evidence under `research/observations/`, `research/network/`, or `research/protocol/` — these directories are **never shipped** and are gitignored for binaries.

2. **Documentation** — Write a short note in `research/notes/` or `research/behavior/` describing the observed behaviour, the evidence, your confidence (Low / Medium / High), and the _implementation implication_ in your own words. Example template is in `research/GUARDRAILS.md`.

3. **Design** — In the PR or a `docs/` ADR, describe the OpenCade design you propose _without referencing proprietary source_. Cite only your observation note and public specs.

4. **Implementation** — Write original Rust / TypeScript from the design. Do not copy decompiled output, do not paraphrase `lib/main.js`, do not look at `fcade.exe` strings while coding.

**Separation:** The person who writes the observation note should ideally not be the same person who writes the final implementation for non-trivial protocol work. If you do both, leave a time gap and do not have proprietary binaries open while implementing.

If in doubt, ask a maintainer before coding.

## What You Must Never Commit

The following will be rejected in CI and in review:

- Proprietary binaries: `fcade.exe`, `fcadefbneo.exe`, `frm.exe`, `Fightcade*.exe`, `ggponet.dll`, `kailleraclient.dll`, any `*.dll` from `D:/Fightcade`.
- Decompiled or transcribed proprietary source, including `fc2-electron/resources/app/lib/main.js` and `lib/static/login.js`.
- Copyrighted ROMs or archives: `*.zip` / `*.7z` / `*.chd` game images, `emulator/*.json` outputs copied as committed TOML without review.
- Proprietary assets: `assets/*.wav` challenge sounds, icons, or other Fightcade media.
- Credentials, API keys, tokens, `.env` with secrets, `%APPDATA%/OpenCade/` dumps.
- `research/binaries/` content.

See `research/GUARDRAILS.md` for the full allow / deny matrix and the local importer workflow for `emulator/*.json` → TOML.

## Code Style & Quality Gates

CI enforces these on every PR. Run them locally before pushing.

### Rust

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
# alternative quick check
cargo check --all-targets
```

- `rustfmt` is authoritative — do not hand-format.
- `clippy` with `-D warnings` must be clean. If you must allow a lint, justify with `#[allow(...)]` and a comment.
- No `unwrap()` / `expect()` in server or relay production paths — propagate with `anyhow` / `thiserror` or `Result`.

### TypeScript / React / Tauri

```powershell
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
```

- `pnpm format` (Prettier) is authoritative for `apps/` and `packages/`.
- ESLint must pass with zero warnings (CI runs `pnpm lint --max-warnings 0`).
- `pnpm typecheck` (`tsc --noEmit`) must pass.

### General

- Keep modules small and independently testable (PRD §43).
- Prefer mature, Apache-2.0 / MIT compatible crates and npm packages. Document any new dependency justification in the PR description.
- Treat all network input as untrusted — validate at the boundary, never `eval` or shell out with remote data.
- Emulator launch arguments must be built from validated templates; never interpolate unsanitized strings into a shell.

## Testing Before a PR

Do not open a PR with failing or missing tests for changed behaviour.

1. **Unit** — protocol serialization, game definitions, adapters, config, state machines.
2. **Integration** — `Client ↔ Server`, `Server ↔ Database`, `Client ↔ Emulator` (mock the emulator process).
3. **Networking** — LAN, same NAT, different NAT, restrictive NAT, packet loss, relay fallback (manual or simulated).
4. **End-to-end** — Register → Login → Join lobby → Challenge → Accept → Session → Launch → Disconnect.

Minimum local verification before pushing:

```powershell
# Rust
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all

# TypeScript
pnpm format
pnpm lint
pnpm typecheck
pnpm test

# If you touched Docker / migrations
docker compose config --quiet
cargo test -p opencade-server --test api_integration  # if present
```

Include in the PR description: which levels you tested and on what OS. For UI changes, attach a screenshot or short clip.

## Pull Request Process

1. **Fork or branch** — `feat/...` from `main`, keep it focused (one feature/fix per PR).
2. **Sync** — Rebase on `main` if it has moved; resolve conflicts locally.
3. **Quality gates** — Run the checks above. CI must be green (fmt, clippy, lint, typecheck, tests, no proprietary blobs).
4. **Describe** — Fill the PR template: problem, solution, clean-room notes (link to `research/notes/...` if applicable), testing, screenshots, breaking changes.
5. **Review** — At least one maintainer approval required. Address review comments with new commits (do not force-push after review has started unless requested).
6. **Merge** — Maintainers squash-merge with a Conventional Commit title. The branch is deleted after merge.

Draft PRs are welcome for early feedback — mark as `Draft` and note what is still missing.

## Reporting Issues & Security

- **Bugs / features:** Use `.github/ISSUE_TEMPLATE/` — include OS, app version, logs (sanitized), and reproduction steps. Never paste tokens or ROM links.
- **Security vulnerabilities:** Do **not** open a public issue. Email the maintainers listed in `SECURITY.md` (or open a private security advisory). Include impact, reproduction, and suggested mitigation if known.
- **Proprietary content take-down:** If you believe any committed file violates clean-room rules, open an issue with label `legal` or contact the maintainers directly — it will be removed promptly.

---

Thank you for building OpenCade the right way — original code, open protocol, self-hostable infrastructure.

P.S. If this project saved you time, consider buying the maintainers a coffee: **https://buymeacoffee.com/zendevve**
