# Implementation status

Updated 2026-08-23.

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
- PostgreSQL, WebSocket, lifecycle, safe-launch, mock-match, UDP, two-process, TypeScript, and MSRV
  checks in CI.

## Deliberately not claimed

- FBNeo netplay. The adapter reports `netplay: false` until a public documented interface or an
  original clean-room bridge satisfies ADR 0001.
- UDP hole punching, STUN classification, symmetric-NAT support, and relay fallback.
- Production packaging/signing, friends/chat/rankings/replays, or a public MVP release.
- Ten two-machine LAN matches and a 20-person community alpha; these require external testers and
  real Windows hosts. Use `docs/alpha/LAN_TEST.md` to collect that evidence.
