# Implementation status

Updated 2026-08-24.

## Implemented and automated

- Composed Axum runtime with automatic SQLx migrations, liveness/readiness, strict production
  configuration, scoped CORS, and redacted errors.
- Argon2 registration/login, hashed opaque sessions, revocation, authenticated REST and WebSocket.
- Durable addressed challenges with ownership rules; transactional room and match lifecycle.
- Bounded/rate-limited WebSocket signaling restricted to authenticated room members with correlated
  acknowledgements and errors.
- Rust-authoritative protocol payloads and generated TypeScript bindings with a CI drift gate.
- React/Tauri login, games, lobby, challenge, room status, reconnect, local availability scan,
  diagnostics, and redacted report export with direct-UDP frame/checksum evidence.
- Safe process abstraction, canonical root checks, `PathBuf`/`OsString` arguments, process tracking,
  and FBNeo local detection/validation/launch.
- Deterministic mock adapter, bounded in-memory input transport, and a nonce-bound direct-UDP match
  runner wired through authenticated endpoint exchange and the desktop match screen.
- A standalone two-node probe CLI plus a real two-process localhost test that verifies identical
  60-frame transcripts and machine-readable reports.
- One canonical, privacy-minimized desktop/CLI evidence format, a fail-closed paired-report
  verifier, and CI-built Windows LAN alpha tools.
- NAT traversal fallback (direct UDP → hole-punch → STUN → WS/relay) via `packages/networking` `NatTraversal`/`FallbackOrder`, STUN hint in `GET /servers` (`stun:host:port`), and latency metrics (EWMA RTT alpha=0.2, loss, jitter) over 30 samples.
- Standalone `opencade-relay` service (Axum WS relay on `/relay` with `GET /health`/`ready`, room bucket forwarding, envelope validation, graceful shutdown) wired in `docker-compose.yml` on `3478` and `3478/udp` with dedicated Dockerfile.
- Desktop `diagnose_network` reports `nat`, `rtt_ms`, `loss`, `jitter_ms`, `relay_reachable`, `stun_reachable` with wired Tauri command and typed `apps/client/src/lib/diagnostics.ts` wrapper.
- PostgreSQL, WebSocket, lifecycle, safe-launch, mock-match, UDP, NAT, relay, two-process, TypeScript, and MSRV
  checks in CI.

## Deliberately not claimed

- FBNeo netplay. The adapter reports `netplay: false` until a public documented interface or an
  original clean-room bridge satisfies ADR 0001.
- Symmetric-NAT traversal beyond classification, TURN allocation, and production STUN/TURN deployment.
- Production packaging/signing, friends/chat/rankings/replays, or a public MVP release.
- Ten two-machine LAN matches and a 20-person community alpha; these require external testers and
  real Windows hosts. Use `docs/alpha/LAN_TEST.md` to collect that evidence.
