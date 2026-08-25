# OpenCade

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-Join%20Chat-5865F2?logo=discord&logoColor=white)](https://discord.gg/Y4rDyTScPe)
![Status](https://img.shields.io/badge/status-proof--of--match-alpha-amber)
![Spec](https://img.shields.io/badge/spec-v0.1-lightgrey)
[![Support OpenCade](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/zendevve)

> **Open-source arcade netplay — a clean-room, community-owned alternative for low-latency rollback matchmaking and emulation.**

OpenCade is a monorepo for a modern arcade netplay platform: Rust server (Axum + PostgreSQL), Tauri + React + TypeScript desktop client, and a pluggable emulator adapter SDK. The repository contains an executable **Proof of Match** control plane, deterministic mock-adapter data plane, safe local FBNeo launch, direct UDP/STUN/hole-punching, authenticated readiness-probe relay fallback, and an experimental native-process RetroArch adapter. Standalone FBNeo netplay, physical cross-NAT results, behavior classification, and real two-machine RetroArch play remain explicitly unproven.

## Support — keep it community-owned

OpenCade exists to replace proprietary Fightcade with a community-owned, self-hostable, and fully auditable alternative. Proof-of-Match is done — lobby, signaling, deterministic transport, STUN discovery, bounded hole punching, authenticated relay fallback, and an experimental user-supplied RetroArch process boundary run today. What remains is physical two-Windows Proof-of-Play evidence and production release hardening.

**[☕ Buy Me a Coffee — https://buymeacoffee.com/zendevve](https://buymeacoffee.com/zendevve)** — one coffee keeps the relay and adapter work open. No paywall, no premium.

---

## Quick Start

### Prerequisites

- Docker + Docker Compose (server + Postgres)
- Rust stable + `sqlx-cli` (server)
- Node.js 24+ + pnpm 11+ (client)
- Rust + Tauri prerequisites ([tauri.app/start/prerequisites](https://tauri.app/start/prerequisites))

### 1. Server (Docker Compose)

```bash
# from repo root
cp .env.example .env
# replace SESSION_SECRET in .env, then:
docker compose up -d

# verify
curl http://localhost:8080/health
docker compose logs -f opencade-server
```

This starts `opencade-server`, PostgreSQL, and the authenticated readiness-probe relay. The server applies committed SQLx migrations before it begins serving.

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
pnpm -C apps/client tauri dev

# or run web-only frontend
pnpm --filter @opencade/client dev

# production bundle
pnpm -C apps/client tauri build
```

> **Ports (default):** server `8080`, PostgreSQL `5432`, client dev `1420`.

---

## Repository Structure

```
OpenCade/
├── apps/
│   ├── client/                 # Tauri + React + TypeScript desktop client
│   │   ├── src/                # Auth, games, lobby, challenge, and match views
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
│   ├── networking/             # Bounded input frames, deterministic in-memory + direct UDP transports
│   └── shared/                 # Cross-cutting utils, logging, config
├── adapters/
│   ├── fbneo/                  # Standalone FBNeo local-play adapter
│   └── retroarch/              # Experimental native-process netplay adapter
├── services/                   # Authenticated bounded readiness-probe relay
├── scripts/alpha/              # Windows alpha-kit packager, verifier, doctor, and launcher
├── research/                   # OBSERVATIONS ONLY — never shipped (see Clean-Room Notice)
│   ├── observations/           # Dated, factual notes from black-box behavior
│   ├── protocol/               # Captured message field notes (no replay)
│   ├── binaries/               # Inventory only — no binaries checked in
│   ├── network/                # RTT / NAT / firewall observations
│   ├── behavior/               # UX flows, state transitions
│   └── notes/                  # Working scratch (not source of truth)
├── docs/
│   ├── ARCHITECTURE.md                 # System architecture & subsystem map (authoritative)
│   ├── adr/                            # Architecture decision records
│   ├── alpha/                          # LAN test and match-report procedures
│   ├── IMPLEMENTATION_STATUS.md        # Verified scope and explicit non-claims
│   └── reference-fightcade-install.md  # D:/Fightcade read-only notes (never copied)
├── docker/
│   └── (compose at root: `docker-compose.yml` — see also `docker/` if present)
├── .github/
│   └── workflows/              # CI (fmt, clippy, test, build)
└── tests/                      # Cross-package integration tests
```

See **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** for the full system design, subsystem boundaries, data model, and adapter contract.

---

## Clean-Room Notice

> **D:/Fightcade is a read-only reference. No proprietary binaries, ROMs, or credentials are shipped in this repository.**

OpenCade is built under a strict clean-room process:

1.  **Observation** — black-box study of behavior and protocols against `D:/Fightcade` as an installed reference. Notes go to `research/` only.
2.  **Documentation** — observations are distilled into specs (`docs/`, `packages/protocol`).
3.  **Design** — new interfaces are designed from the spec, not from decompiled or copied code.
4.  **Implementation** — original code only.

**Forbidden (never committed):** proprietary binaries, ROMs/assets, credentials/tokens, decompiled or copy-pasted code, packet dumps with user data.

**Allowed:** original source under Apache-2.0, documentation, licensed dependencies, public specifications.

The `research/` directory is workspace-only and is **not shipped** in any release artifact or the server container image. The process and prohibited material are documented in `research/GUARDRAILS.md` and `docs/ARCHITECTURE.md §18`. CI enforces the guardrail (`research/` is excluded from builds and binary scans block proprietary artifacts).

---

## Architecture

High-level: `Client (Tauri)` ↔ `Server (Axum REST + authenticated WebSocket)` for the control plane; direct UDP or the deterministic in-memory transport carries OpenCade input frames; the adapter boundary owns safe local emulator execution.

- **Server:** auth (Argon2id), hashed sessions, games, server hints, lobbies, durable challenges,
  rooms/matches, and authenticated WebSocket signaling (`offer`/`answer`/`candidate`).
- **Networking:** deterministic in-memory, direct UDP, RFC 8489 discovery, nonce-bound hole punching,
  and a short-lived room-member-ticket WebSocket relay are implemented. Physical cross-NAT evidence
  and cone/symmetric behavior classification remain pending.
- **Client:** login/register, games, lobby challenges, match state, local availability scan, diagnostics, and redacted report export; Rust core owns process spawn, filesystem validation, and diagnostics.
- **Emulator SDK:** explicit OpenCade/native/blocked netplay modes, safe process launch, ROM validation,
  game-definition TOML, and an experimental user-supplied RetroArch + FBNeo-core adapter.

Full reference: **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

---

## Roadmap — Phases M0–M7

| Phase  | Milestone             | Focus                                                                                                                                  | Exit Criteria                                                 |
| ------ | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| **M0** | Scaffolding           | Monorepo, CI, Docker Compose, lint/fmt, clean-room guardrails                                                                          | `pnpm install && docker compose up -d` works from clean clone |
| **M1** | Server Core           | Axum + PostgreSQL, auth (register/login/logout, Argon2id), users/sessions, health/observability                                        | REST auth + health passes integration tests                   |
| **M2** | Realtime & Networking | WebSocket signaling, room state machine (`WAITING`→`PLAYING`→`ENDED`/`CANCELLED`), challenge flow                                      | Signaling versioned protocol + presence/chat e2e              |
| **M3** | Client Shell          | Tauri + React shell, routing (Games/Lobbies/Friends/Servers/Settings), Rust fs/process/logging                                         | Client launches, talks to server, diagnostics panel           |
| **M4** | Emulator SDK          | Adapter trait (`detect`/`validate`/`getVersion`/`launch`/`stop`/`configure`/`getSupportedGames`), FBNeo adapter, TOML game definitions | Local ROM scan + safe launch for one title                    |
| **M5** | Matchmaking           | Lobbies, game versions, server browser, matchmaking & room lifecycle                                                                   | Create/join/spectate room e2e with two peers                  |
| **M6** | NAT & Relay           | STUN, hole-punching, authenticated `opencade-relay` WebSocket fallback, RTT/loss/jitter, Network Test                                  | Direct + relayed matches measured; relay Docker image         |
| **M7** | MVP Release           | Hardening, bans/reports, replay hooks, packaging, docs                                                                                 | Tagged MVP, signed artifacts, no proprietary content          |

---

## Community

**Discord — where we discuss OpenCade and everything around it (not only OpenCade):** https://discord.gg/Y4rDyTScPe

General dev chat, architecture questions, emulator adapter ideas, and matchmaking talk — all in one place.

If OpenCade helped you host a match, consider supporting the build: **https://buymeacoffee.com/zendevve**

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) — and if you can't contribute code, sponsorship via [Buy Me a Coffee](https://buymeacoffee.com/zendevve) keeps CI, docs, and LAN testing funded.

## Security

Do not open public issues for sensitive vulnerabilities. See `SECURITY.md` for disclosure.

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 OpenCade Contributors.
