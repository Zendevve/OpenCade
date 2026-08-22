# OpenFight

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
![Status](https://img.shields.io/badge/status-MVP%20spec-orange)
![Spec](https://img.shields.io/badge/spec-v0.1-lightgrey)

> **Open-source arcade netplay — a clean-room, community-owned alternative for low-latency rollback matchmaking and emulation.**

OpenFight is a monorepo for a modern arcade netplay platform: Rust server (Axum + PostgreSQL), Tauri + React + TypeScript desktop client, and a pluggable emulator adapter SDK. This repository is currently at **MVP spec** stage — architecture, protocols, and interfaces are defined; implementation follows the phased roadmap below.

---

## Quick Start

### Prerequisites

- Docker + Docker Compose (server + Postgres + relay)
- Rust stable + `sqlx-cli` (server)
- Node.js 20+ + pnpm 9+ (client)
- Rust + Tauri prerequisites ([tauri.app/start/prerequisites](https://tauri.app/start/prerequisites))

### 1. Server (Docker Compose)

```bash
# from repo root
docker compose up -d

# verify
curl http://localhost:8080/health
docker compose logs -f openfight-server
```

This starts `openfight-server` + `postgres` + `openfight-relay` as defined in `docker-compose.yml`.

```bash
# stop
docker compose down

# reset DB (destructive)
docker compose down -v
```

### 2. Client (Tauri + React)

```bash
# install JS dependencies
pnpm install

# run desktop app in dev mode (Vite + Tauri)
pnpm tauri dev

# or run web-only frontend
pnpm --filter @openfight/client dev

# production bundle
pnpm tauri build
```

> **Ports (default):** server `8080`, relay `4000/udp`, client dev `1420`, Vite HMR `1421`.

---

## Repository Structure

```
OpenFight/
├── apps/
│   ├── client/                 # Tauri + React + TypeScript desktop client
│   │   ├── src/                # Routes: Games / Lobbies / Friends / Servers / Settings
│   │   ├── src-tauri/          # Rust native layer (process / fs / logging / diagnostics)
│   │   └── package.json
│   └── server/                 # Rust + Axum + PostgreSQL API + WebSocket signaling
│       ├── src/
│       ├── migrations/
│       └── Dockerfile
├── packages/
│   ├── protocol/               # Versioned signaling + REST contract (shared types)
│   ├── emulator-sdk/           # Adapter trait: detect / validate / getVersion / launch / stop / configure
│   ├── game-definitions/       # Declarative TOML (id, name, emulator, launch args, validation)
│   ├── networking/             # NAT traversal, RTT/loss/jitter, room state machine
│   └── shared/                 # Cross-cutting utils, logging, config
├── adapters/
│   └── fbneo/                  # FBNeo reference adapter (first implementation)
├── services/
│   └── relay/                  # openfight-relay (TURN-like UDP relay fallback)
├── research/                   # OBSERVATIONS ONLY — never shipped (see Clean-Room Notice)
│   ├── observations/           # Dated, factual notes from black-box behavior
│   ├── protocol/               # Captured message field notes (no replay)
│   ├── binaries/               # Inventory only — no binaries checked in
│   ├── network/                # RTT / NAT / firewall observations
│   ├── behavior/               # UX flows, state transitions
│   └── notes/                  # Working scratch (not source of truth)
├── docs/
│   ├── ARCHITECTURE.md         # System architecture & subsystem map
│   ├── PROTOCOL.md             # Signaling & API specification
│   └── CLEAN_ROOM.md           # Observation → Documentation → Design → Implementation
├── docker/
│   └── compose.yml             # (or ./docker-compose.yml at root)
├── .github/
│   └── workflows/              # CI (fmt, clippy, test, build)
└── tests/                      # Cross-package integration tests
```

See **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** for the full system design, subsystem boundaries, data model, and adapter contract.

---

## Clean-Room Notice

> **D:/Fightcade is a read-only reference. No proprietary binaries, ROMs, or credentials are shipped in this repository.**

OpenFight is built under a strict clean-room process:

1.  **Observation** — black-box study of behavior and protocols against `D:/Fightcade` as an installed reference. Notes go to `research/` only.
2.  **Documentation** — observations are distilled into specs (`docs/`, `packages/protocol`).
3.  **Design** — new interfaces are designed from the spec, not from decompiled or copied code.
4.  **Implementation** — original code only.

**Forbidden (never committed):** proprietary binaries, ROMs/assets, credentials/tokens, decompiled or copy-pasted code, packet dumps with user data.

**Allowed:** original source under Apache-2.0, documentation, licensed dependencies, public specifications.

The `research/` directory is workspace-only and is **not shipped** in any release artifact or container image. CI enforces the guardrail (`research/` is excluded from builds and binary scans block proprietary artifacts). Details in `docs/CLEAN_ROOM.md`.

---

## Architecture

High-level: `Client (Tauri)` ↔ `Server (Axum REST + WebSocket)` ↔ `Relay (UDP)` ↔ `Emulator (adapter-launched)`.

- **Server:** auth (Argon2id), sessions, games/versions, servers, rooms/matches, reports/bans, WebSocket signaling (`offer`/`answer`/`candidate`, `presence.update`, `chat.message`, `challenge.*`).
- **Networking:** direct UDP → hole-punching (STUN) → relay (TURN) fallback; RTT/loss/jitter telemetry; Network Test diagnostics.
- **Client:** routes `Games / Lobbies / Friends / Servers / Settings`; Rust core for process spawn, filesystem, and diagnostics; React frontend for UI.
- **Emulator SDK:** trait-based adapters with safe process launch (no shell injection, argument escaping), ROM validation, and game-definition TOML.

Full reference: **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

---

## Roadmap — Phases M0–M7

| Phase | Milestone | Focus | Exit Criteria |
|-------|-----------|-------|---------------|
| **M0** | Scaffolding | Monorepo, CI, Docker Compose, lint/fmt, clean-room guardrails | `pnpm install && docker compose up -d` works from clean clone |
| **M1** | Server Core | Axum + PostgreSQL, auth (register/login/logout, Argon2id), users/sessions, health/observability | REST auth + health passes integration tests |
| **M2** | Realtime & Networking | WebSocket signaling, room state machine (`WAITING`→`PLAYING`→`ENDED`/`CANCELLED`), challenge flow | Signaling versioned protocol + presence/chat e2e |
| **M3** | Client Shell | Tauri + React shell, routing (Games/Lobbies/Friends/Servers/Settings), Rust fs/process/logging | Client launches, talks to server, diagnostics panel |
| **M4** | Emulator SDK | Adapter trait (`detect`/`validate`/`getVersion`/`launch`/`stop`/`configure`/`getSupportedGames`), FBNeo adapter, TOML game definitions | Local ROM scan + safe launch for one title |
| **M5** | Matchmaking | Lobbies, game versions, server browser, matchmaking & room lifecycle | Create/join/spectate room e2e with two peers |
| **M6** | NAT & Relay | STUN, hole-punching, `openfight-relay` TURN fallback, RTT/loss/jitter, Network Test | Direct + relayed matches measured; relay Docker image |
| **M7** | MVP Release | Hardening, bans/reports, replay hooks, packaging, docs | Tagged MVP, signed artifacts, no proprietary content |

---

## Contributing

See `CONTRIBUTING.md` (coming in M0). By contributing you agree to the clean-room rules and that your contributions are licensed under Apache-2.0.

## Security

Do not open public issues for sensitive vulnerabilities. See `SECURITY.md` for disclosure.

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 OpenFight Contributors.
