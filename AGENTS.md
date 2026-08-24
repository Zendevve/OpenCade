# Repository Guidelines

> **Community:** https://discord.gg/Y4rDyTScPe — where we discuss OpenCade and everything around it (not only OpenCade).

## Project Overview

OpenCade — open-source arcade netplay platform, clean-room alternative to proprietary Fightcade. Monorepo at `D:/OpenCade` (Apache-2.0) with Tauri + React + TypeScript client and Rust + Axum + PostgreSQL server. `D:/Fightcade` (v2.1.45) is **read-only reference** — never copied into this repo (see `docs/ARCHITECTURE.md §2` and `docs/reference-fightcade-install.md`). Goal: lobby → challenge → versioned signaling → P2P (or WS relay) → safe emulator launch.

## Architecture & Data Flow

- **System:** `apps/client` (Tauri) ↔ `apps/server` (Axum) over HTTPS/WS ` /ws` → P2P/relay between peers. Server never sees game inputs except when relaying as fallback.
- **Client:** Tauri 2.x hosts React routes `Games | Lobbies | Friends | Servers | Settings`. Rust core owns `fs` (ROM scan under `emulator/<core>/ROMs/`), `process` (safe direct process spawning with arg escaping, no shell), `diag` (Network Test). State: TanStack Query + Zustand, WS client with typed `packages/protocol` discriminated unions and reconnect backoff.
- **Server:** Single Axum monolith + `postgres:16-alpine` (no Redis in MVP). Routes `POST /api/v1/auth/*`, `GET /api/v1/games`, `GET /api/v1/lobbies/:game`, `POST /api/v1/rooms` + `.../:id/{accept,decline,cancel}`, WS `/ws` with versioned envelope `{type,version,request_id,timestamp,payload}` (`presence.update`, `chat.message`, `challenge.*`, `session.offer/answer/candidate`, `room.*`). In-memory presence Hub, Postgres for durable state.
- **Networking:** Signaling relayed verbatim through server; client-driven UDP hole-punch → STUN hint (`GET /servers` returns `stun:host:port` when configured) → WS relay fallback (in-process `relay` module, future `services/relay` binary). Room states `WAITING → CHALLENGING → CONNECTING → PLAYING → FINISHED|CANCELLED`. Latency `rtt_ms/loss/jitter` via `presence.update`.
- **Reference only:** `docs/reference-fightcade-install.md` describes the opaque PyInstaller launcher (`emulator/fcade.exe`/`frm.exe` → `fightcade/launcher.py`) and `fbneo-training-mode` Lua surface — not used at runtime.

## Key Directories

- `apps/client/` — Tauri app (replaces `fc2-electron`). `src/routes/{Games,Lobbies,Friends,Servers,Settings}.tsx`, `src/components/*`, `src/lib/{api,ws,store}.ts`, `src-tauri/src/{main.rs,commands/{fs,process,diag}.rs,adapters/fbneo.rs}`, `tauri.conf.json` (least-privilege `fs`/`process` allowlist).
- `apps/server/` — Axum monolith (replaces `fightcade.com/replay`). `src/{main.rs,routes/*,ws.rs,state.rs,auth.rs,models/*}`, `migrations/001_users.sql`, `Dockerfile`.
- `packages/protocol/` — shared wire types (`Envelope`, `RoomState`, `PresenceState`) — single source of truth, `serde` + `ts-rs` generation.
- `packages/emulator-sdk/` — `pub trait EmulatorAdapter { detect/validate/get_version/launch/stop/configure }` + `LaunchCtx`, `ChildHandle`. No shell, path canonicalization + prefix check.
- `packages/game-definitions/` — declarative `games/*.toml` (`schema_version=1`, `id`, `name`, `emulator="fbneo"`, `[launch] args=["{rom}"]`, `[validation] required_files=["neogeo.zip"]`) + `src/loader.rs` + legacy `emulator/*.json` → TOML importer (build-time only).
- `adapters/fbneo/` — only required adapter in MVP (`fcadefbneo.exe` detection, `fcadefbneo.default.ini` version check, safe arg building). Future `flycast`/`snes9x` behind feature flags.
- `services/relay/` — placeholder crate `opencade-relay` (future STUN/TURN); not required for MVP (in-process WS relay suffices).
- `research/` — **not shipped** — `observations/`, `protocol/`, `binaries/` (gitignored), `network/`, `behavior/`, `notes/` + `GUARDRAILS.md`. Keep `research/binaries/.gitkeep`.
- `docs/` — `ARCHITECTURE.md` (authoritative), `reference-fightcade-install.md` (read-only install notes).
- `tests/`, `docker/`, `.github/workflows/` — integration tests, compose overlay, CI.
- Reference (read-only, outside repo): `D:/Fightcade/emulator/fbneo/fbneo-training-mode/` — vendored Lua `games/<rom>/<rom>.lua` + `hitboxes/*.lua` + `Run()` hook; pattern only.

## Development Commands

```bash
# prerequisites: Rust 1.98+, Node 24+, pnpm 11+, Docker, Postgres 16
pnpm install                    # workspace install (apps/*, packages/*)
pnpm -C apps/client tauri dev   # Vite + Tauri dev (Windows)
pnpm -C apps/client build       # TS check + Vite build
pnpm format && pnpm lint        # prettier + eslint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p opencade-server -- --migrate   # sqlx migrate run
cargo run -p game-defs-import -- D:/Fightcade/emulator/fbneo_roms.json --out packages/game-definitions/games

# infra
docker compose -f docker/docker-compose.yml up --build -d   # or docker-compose.yml at root
curl http://localhost:8080/health && curl http://localhost:8080/ready
psql $DATABASE_URL -f apps/server/migrations/001_users.sql

# diagnostics
pnpm tauri dev -- --log opencade  # client logs to %APPDATA%/OpenCade/logs/opencade.log
```

## Code Conventions & Common Patterns

- **Formatting:** `cargo fmt` (rustfmt edition 2021, `max_width=100`) and `pnpm format` (Prettier) — CI blocks on `fmt --check`. `clippy -D warnings` required.
- **Naming:** crates `opencade-*`, TS packages `@opencade/*`, adapters `fbneo` (kebab), TOML ids `sfiii3`, `kof98` (snake, lowercase). DB tables `snake_case`; API/WS JSON also `snake_case` via `#[serde(rename_all = "snake_case")]` for payload keys and `#[serde(rename = "type")]` for the `type` field (see `Envelope` in `apps/server/src/main.rs:17-24` and `packages/protocol`).
- **Error handling:** `thiserror` + `anyhow` in Rust, never `unwrap()` in server paths; TS `Result<T,E>`-style returns from `src/lib/api.ts`. Structured `tracing` logs (JSON in prod, pretty in dev) — never log tokens, passwords, or ROM paths with PII.
- **Async:** Tokio everywhere server-side; Tauri commands `async` with `#[tauri::command(async)]`; WS client uses `tokio-tungstenite` + backoff. No blocking in async context.
- **Protocol:** every WS message `Envelope {type:"signaling.offer", version:"1.0", request_id, timestamp, payload:{room_id,candidate}}` with `version` as **string** `"1.0"` canonical (compat `"1"` accepted) matching `pub const PROTOCOL_VERSION: &str = "1.0"` and `is_supported_version` in `packages/protocol/src/lib.rs:14-21` / `pub version: String` in `apps/server/src/main.rs:20`. Server validates `version` then `type`, returns `{code:"unknown_type"}` for unknown `type`, `{code:"version_unsupported"}` for unsupported version; forward-compatible bump via `"2.0"` handler.
- **Process launch:** `Command::new(exe).args(escaped_os_strings).current_dir(exe.parent()).spawn()` — canonicalize exe, verify under allowlist root, reject `..` traversal, no `cmd /C`, `extra_env` allowlisted only. `ChildHandle` tracks pid, streams stdout/stderr to `logs/emulator.log`.
- **Adapter contract:** `detect()` checks `emulator/fbneo/fcadefbneo.exe` + `fcadefbneo.default.ini`; `validate()` checks `required_files` existence and warns on version mismatch vs `VERSION.txt 2.1.45`; `launch(ctx)` builds `LaunchCtx{exe, rom:PathBuf, args:Vec<OsString>}`; `stop(handle)` graceful `CTRL+C` → `kill` after timeout.
- **Game defs:** `schema_version` mandatory (MVP `1`), loader rejects unknown `schema_version`, `id` must be `^[a-z0-9_]{3,20}$`, `launch.args` is `Vec<String>` with `{rom}` substitution via `OsString` positional replacement — no string concat.
- **Clean-room:** `research/` is observation-only, never compiled. `cargo deny` + `license = "Apache-2.0"` allowlist; no GPL emulator cores linked. Citations for `fbneo-training-mode` inspiration in `NOTICE`.

## Important Files

- `docs/ARCHITECTURE.md` — authoritative system boundaries, diagram, M0-M7 phases, guardrails (read before coding)
- `docs/reference-fightcade-install.md` — read-only notes on `D:/Fightcade` (Electron wrapper, PyInstaller launcher, training-mode Lua, JSON catalogs)
- `research/GUARDRAILS.md` — forbidden/allowed lists, Observation→Documentation→Design→Implementation process
- `apps/client/src-tauri/tauri.conf.json` — Tauri permissions (`fs:allow-read-dir ROMs`, `process:allow-spawn` only known binaries, `store:allow`)
- `apps/client/src/routes/Games.tsx` — game list (derived from `game-defs` + local scan) + challenge flow
- `apps/server/src/main.rs` + `apps/server/src/ws.rs` — Axum router + WS versioned envelope relay
- `apps/server/migrations/001_users.sql` — `users(id,username,password_hash)`, `sessions`, `games`, `rooms`, `matches`, `chat_messages`
- `packages/protocol/src/lib.rs` — `Envelope`, `RoomState`, `PresenceState` (source of truth)
- `packages/emulator-sdk/src/lib.rs` — `EmulatorAdapter` trait
- `packages/game-definitions/games/sfiii3.toml` — example declarative game (template for new games)
- `docker-compose.yml` + `apps/server/Dockerfile` + `.env.example` (`DATABASE_URL`, `SESSION_SECRET`)
- `pnpm-workspace.yaml`, `rustfmt.toml`, `.clippy.toml`, `.github/workflows/ci.yml`

## Runtime/Tooling Preferences

- **Runtime:** Rust 1.98+ (MSRV), Tauri 2.x (WebView2 on Windows), Node 24+ with **pnpm 11** (not npm/yarn), Postgres 16-alpine (sqlx compile-time checked). No Bun. No Electron.
- **Package manager:** `pnpm` at root (`pnpm-workspace.yaml` covers `apps/*`, `packages/*`, `adapters/*`, `services/*`) — `pnpm install` only, commit `pnpm-lock.yaml`.
- **Build:** `cargo build --workspace`, `pnpm -C apps/client build`, `tauri build` (MSI/NSIS on Windows). Docker multi-stage `rust:1.98-bookworm → debian:bookworm-slim`.
- **Env:** `DATABASE_URL=postgres://opencade:opencade@db:5432/opencade`, `SESSION_SECRET` (32B CSPRNG), `RUST_LOG=info,opencade_server=debug`. Never commit `.env` (see `.env.example`).
- **OS:** Windows 10/11 primary (Tauri), Linux/macOS viable via same stack — no `.lnk` shortcuts, use `tauri::path`.
- **Tooling constraints:** keep `disableDevTools` off in dev, on in prod via Tauri `tauri.conf.json > build > devPath`. No `shell` permission in `tauri.conf.json`; use `process` allowlist.

## Testing & QA

- **No ROMs/binaries in tests:** use fixtures under `tests/fixtures/` (tiny TOML, mock adapter). `research/binaries/` is gitignored.
- **Unit:** `cargo test -p opencade-protocol -- envelope serde`, `-p emulator-sdk -- arg escaping`, `-p game-definitions -- loader/scan`, `pnpm test` for `packages/shared` (Vitest).
- **Integration:** `cargo test --workspace -- --ignored` (spins `Axum` + `postgres` via `docker compose up -d db`, registers two users, WS presence → `challenge.send/accept` → `signaling.offer/answer/candidate` → `room PLAYING` → disconnect).
- ** networking:** LAN, same NAT, different NAT, symmetric NAT (expect relay fallback), packet loss/latency injection via `tc`; `diagnostics:network_test` command asserts `nat:cone|symmetric`, `rtt_ms`, `relay_reachable`.
- **Manual QA loop (MVP):** `docker compose up -d` → `curl /health` → `pnpm tauri dev` → login → `Games` shows owned `sfiii3` (needs `sfiii3.zip` + `neogeo.zip` under `emulator/fbneo/ROMs/` scanned locally) → challenge peer in `Lobbies/:gameId` → accept → `CONNECTING` (P2P or relay) → emulator spawns `fcadefbneo.exe` with escaped `C:\path with spaces\sfiii3.zip` → play → `FINISHED` → export logs `Settings → Export Logs`.
- **CI gate:** `.github/workflows/ci.yml` (`cargo fmt --check`, `clippy -D warnings`, `cargo test`, `pnpm format:check`, `pnpm build`, `docker compose config` lint). Workflow push requires `workflow` scope — see `docs/ARCHITECTURE.md §16`.
