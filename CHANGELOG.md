# Changelog

All notable changes to OpenCade are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-24

First auditable vertical slice: two authenticated users can challenge, negotiate transport, run a deterministic direct-UDP match, and produce a correlated, privacy-minimized report. See `docs/ARCHITECTURE.md` (authoritative), `docs/adr/0001-proof-of-match-boundaries.md`, and `docs/IMPLEMENTATION_STATUS.md`.

### Added

- **Runtime composition** — Axum `build_app(state)` with automatic `sqlx::migrate!` on boot and `--migrate` CLI, liveness `GET /health` and readiness `GET /ready` (DB-checked, redacted errors), strict production `SESSION_SECRET` (>=32 chars) and `STUN_*`/`ALLOWED_ORIGINS` validation, scoped CORS, structured `tracing` logs.
- **Auth** — Argon2id registration/login (`POST /api/v1/auth/*`), SHA-256-hashed opaque sessions, `AuthUser` extractor, revocation/expiry, authenticated REST and `GET /ws` (query `token` + bounded queues/rate-limit, envelope validation).
- **Challenges & rooms** — Durable `challenges` table with owner rules (create/accept/decline/cancel), transactional room and `matches` lifecycle (`WAITING -> CHALLENGING -> CONNECTING -> PLAYING -> FINISHED|CANCELLED`), pure `room_state` transition function with unit tests.
- **Protocol** — Rust-authoritative `packages/protocol` envelope `{type, version, request_id, timestamp, payload}` (canonical "1.0", compat "1"), `PROTOCOL_VERSION` + `is_supported_version`, `ts-rs` generated TypeScript under `packages/protocol/src/generated/` with CI drift gate. Payloads: `presence.update`, `chat.message`, `challenge.*`, `session.offer/answer/candidate`, `match.endpoint`, `match.probe_completed`, `room.state`.
- **Client** — React + Tauri 1.x shell (`apps/client`): login/register, games list with local availability, lobby presence, challenge send/accept, connecting/playing/finished screen, typed REST (`lib/api.ts`) and WS (`lib/ws.ts`) with reconnect backoff and `request_id` correlation, diagnostics button and redacted report export.
- **Emulator SDK** — `packages/emulator-sdk` trait `EmulatorAdapter` + `LaunchSpec`, `ProcessLauncher` abstraction, `PathBuf`/`OsString` args, `canonicalize_below` root check, no shell, `ChildHandle` tracking, `MockAdapter` with `MatchDescriptor` and explicit `AdapterCapabilities`.
- **FBNeo adapter** — `adapters/fbneo` detection of `emulator/fbneo/fcadefbneo.exe`, version check, `required_files` validation, safe `build_command` with per-arg sanitization, traversal rejected.
- **Game definitions** — `packages/game-definitions` declarative TOML (`schema_version=1`, `id ^[a-z0-9_]{3,20}$`, `launch.args` with `{rom}` placeholder), JSON schema, loader, and build-time importer.
- **Networking data plane** — `packages/networking`: deterministic in-memory transport + nonce-bound direct UDP (`UdpPeer`, `MatchProbeConfig`, 60-frame transcript + FNV checksum), `NatTraversal`/`FallbackOrder` (direct UDP -> hole-punch 3x500ms -> STUN -> WS/relay), `diagnose_network` reporting `nat`, `rtt_ms` (EWMA alpha 0.2 over 30 samples), `loss`, `jitter_ms`, `relay_reachable`, `stun_reachable`, and `stun:host:port` hint in `GET /servers`.
- **Relay** — Standalone `services/relay` (`opencade-relay`) Axum WS relay on `/relay` with `GET /health`/`ready`, room-bucket forwarding, envelope validation, graceful shutdown; wired in `docker-compose.yml` on `3478`/`3478:udp`.
- **LAN evidence kit** — Canonical redacted `MatchReport` (schema v1) from desktop and CLI probe (`opencade-match-probe`/`opencade-match-verify`), fail-closed paired verifier, two-process localhost test verifying identical 60-frame transcripts, and `docs/alpha/LAN_TEST.md` manual gate (10/10 loopback verified on CI).
- **Desktop diagnostics** — `diagnose_network` + `diagnose_adapter` + `diagnose_roms` + `get_logs`, typed wrapper `apps/client/src/lib/diagnostics.ts`, Tauri allowlist (no `shell.open`, restrictive CSP), `withGlobalTauri: false`.
- **CI** — `ci.yml`: Rust `fmt --check` + `clippy -D warnings` + `cargo test --workspace` (Postgres 16), `pnpm format:check`/`typecheck`/`test`/`build`, bindings drift, `docker compose config` lint, Windows MSRV + `opencade-lan-tools-windows` artifact.

### Changed

- Server routing consolidated from `main.rs` stubs into reusable `build_app` wiring `config`, `state`, `routes`, and `ws`. All SQL aligned to canonical schema (TEXT game ids, `host_user_id`, uppercase-at-rest state). Tauri `shell.open` removed, CSP set, production secret checks enforced.

### Fixed

- Schema drift (slug/UUID/lowercase states) eliminated; DB errors no longer masked as seeded responses. UDP probe startup races and Windows `WSAECONNRESET 10054` handled.

### Not claimed (deferred)

- FBNeo netplay (`netplay: false` until ADR 0001 bridge), symmetric-NAT TURN allocation and production STUN/TURN deployment, production packaging/signing, friends/chat/rankings/replays, or public MVP release beyond verifiable LAN direct-UDP proof.

[0.1.0]: https://github.com/opencade/opencade/releases/tag/v0.1.0
