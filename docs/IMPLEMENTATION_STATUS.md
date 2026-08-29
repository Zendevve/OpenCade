# Implementation status

Updated 2026-08-28.

## Implemented and automated

- Composed Axum runtime with automatic SQLx migrations, liveness/readiness, strict production
  configuration, scoped CORS, and redacted errors.
- Argon2 registration/login, hashed opaque sessions, revocation, authenticated REST and WebSocket.
- Per-identity and bounded global authentication throttles, equal-cost unknown-user password work,
  operating-system credential storage for desktop sessions, and a production CSP that requires TLS
  for non-loopback control planes.
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
- RFC 8489 reflexive-address discovery and bounded authenticated UDP hole punching on the same
  reserved socket, with host/reflexive candidate evidence in redacted reports.
- A fail-closed campaign summarizer that derives the 8-of-10 alpha gate and compatibility matrix
  from paired reports, separating direct-UDP and authenticated-relay outcomes.
- Privacy-minimized failure evidence for endpoint, relay, transcript, room-transition, and native
  launch failures, so abandoned rooms count against the campaign instead of disappearing.
- A standalone `opencade-relay` service with health/readiness routes and bounded room forwarding,
  plus server-provided STUN hints and 30-sample latency metrics.
- Short-lived HMAC relay tickets issued only to active room members, immutable room scoping,
  two-peer limits, bounded queues/frames, and automatic desktop readiness-probe fallback.
- A process-boundary RetroArch + FBNeo-core adapter with explicit `NativeProcess` capability,
  safe host/guest argument construction, and SHA-256 executable/core/content fingerprints.
- Server-derived one-use native launch grants, exactly-two-player rooms, a deterministic client
  match coordinator, supervised/reaped child processes, and launch/exit-backed room transitions.
- One-use room invite codes, reconnect-safe revisioned room snapshots, fail-closed two-peer
  compatibility preflights, and an idempotent synchronized launch barrier.
- A community-alpha dashboard backed by authenticated, privacy-filtered, conflict-safe evidence
  uploads and a bounded server-side campaign summary.
- A reusable match-readiness gate that validates the desktop runtime, control plane, local game
  runtime, native port, and advisory network fallback before a player enters a lobby.
- Explicitly opt-in product telemetry for the game-selection-to-lobby activation funnel, with a
  closed Rust/TypeScript event contract, anonymous tab sessions, idempotent ingestion, 90-day raw
  retention, 30-day aggregates, and small-cohort blocker suppression.
- A capability-scoped native TCP tunnel primitive with bounded binary frames and a real loopback
  relay integration test. It is separately ticketed from readiness probes and remains disabled as
  an automatic gameplay route pending physical two-host validation.
- A truthful route gate: same-LAN host candidates may launch the RetroArch TCP alpha; UDP reflexive
  and relay results are readiness-only until a native transport bridge exists.
- Strict playable-report verification that requires matching, well-formed native compatibility
  fingerprints (`opencade-match-verify --require-compatibility`).
- Runtime-configurable desktop server/STUN endpoints and a CI-built Windows desktop alpha executable
  with SHA-256 checksums alongside the probe/verifier/summarizer tools.
- A flat Windows alpha kit with a CI-exercised PowerShell packager/verifier, machine doctor,
  runtime configuration launcher, and dedicated report directory.
- Desktop network diagnostics backed by real RFC 8489 Binding when a STUN endpoint is configured,
- A deterministic two-peer `MockAdapter` + `InMemoryPeer` proof-of-match scenario that drives
  the full 60-frame deterministic transcript and emits canonical redacted `MatchReport`
  evidence verified by the existing pair verifier (`crates/proof-of-match` + `opencade-proof-of-match`
  bin + `crates/proof-of-match/tests/proof_of_match.rs`).
- PostgreSQL, WebSocket, lifecycle, safe-launch, mock-match, proof-of-match, UDP, NAT, relay,
  two-process, TypeScript, and MSRV checks in CI.
- An original Apache-2.0 deterministic libretro test core plus inert `.ocade` content, built and
  signed with the Windows alpha kit, for a complete Proof-of-Play path without ROMs, BIOS files,
  firmware, or a third-party core.
- Controller and deterministic player-slot preflight in the synchronized launch barrier.
- Canonically verified, idempotent post-match receipts that create a fresh one-use same-game invite.
- A k-anonymous public compatibility aggregate and static evidence map that never exposes raw
  reports, identities, endpoints, paths, hashes, or invite data.
- A per-game native route policy that fails closed to direct LAN and selects the TCP tunnel only
  with relay configuration, explicit operator enablement, a bounded physical-attempt minimum, and
  an 80% canonical verified-pair rate.

## Deliberately not claimed

- Standalone FBNeo netplay. That adapter reports `BlockedNoPublicInterface`; ADR 0002 records the
  public-source feasibility result. The separate RetroArch native-process path has no physical
  two-machine evidence yet and remains experimental under ADR 0003.
- Cone/symmetric NAT behavior classification and physically proven relay fallback. One RFC 8489 Binding response is
  deliberately reported only as `open`, `mapped`, or `unknown`.
- TURN allocation and production STUN/TURN deployment.
- Production installer packaging/signing, friends/chat/rankings/replays, or a public MVP release.
- Ten two-machine LAN matches and a 20-person community alpha; these require external testers and
  real Windows hosts. Use `docs/alpha/LAN_TEST.md` to collect that evidence.
- Evidence-qualified automatic TCP-tunnel selection has no physical two-host evidence yet. It is
  implemented but operator-disabled by default under ADR 0006; enabling it without the documented
  campaign would be an unsupported deployment claim.
