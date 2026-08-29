# OpenCade current priorities

> Reviewed: 2026-08-28. This is the active execution queue. Historical opportunity analyses remain
> available in Git history and [`OPPORTUNITIES_2026-08-28.md`](OPPORTUNITIES_2026-08-28.md).

## Verified baseline

OpenCade has an authenticated room/challenge flow, versioned signaling, bounded UDP/relay probes,
RetroArch launch authorization, deterministic proof-of-match evidence, controller preflight,
privacy-safe compatibility aggregates, continuation receipts, and a no-ROM libretro test core.
Rust workspace tests, PostgreSQL integration tests, desktop tests, TypeScript tests, Clippy,
dependency audits, production builds, Compose validation, and workflow linting pass locally.

Physical two-host Windows/NAT evidence remains an external product gate. Do not claim broad
compatibility or Internet playability from loopback tests.

## P0 — deterministic vertical alpha certification

Build one Windows CI command that launches two packaged clients with the deterministic no-ROM core,
drives the real API/signaling/preflight/barrier/native-launch path, uploads both reports, and verifies
a continuation receipt.

Exit criteria:

- observable readiness conditions; no timing-only sleeps;
- real packaged binaries and test core, with no ROM/emulator assets;
- one repeatable loopback happy path blocks release regressions;
- fewer than ten new files and a documented local reproduction command.

Kill criterion: re-scope if the first deterministic loopback path exceeds seven engineering days.
Do not add NAT simulation or a new test framework before the loopback slice works.

## P1 — protocol evolution and typed boundaries

- Decide the backward-compatible path from protocol `"1.0"` after required controller/snapshot
  fields were added. Add old-client/new-server and new-client/old-server contract tests.
- Generate a message registry mapping name, payload, direction, and version from Rust. Use it for
  TypeScript unions, client sends, fixtures, exports, and reference documentation.
- Add runtime decoders to remaining HTTP and Tauri IPC boundaries; launch-critical room snapshots
  are already validated.

## P1 — production topology and release safety

- Explicitly support one control-plane and one relay replica until cross-instance delivery,
  ticket replay protection, session logout, and room affinity are designed.
- Publish immutable server/relay images with digests, SBOMs, and attestations. Until then, container
  deployment means building and pinning the audited Git tag.
- Test migrations from the previous released schema with representative data and lock budgets.
- Split database credentials into a DDL migrator role and least-privilege runtime role.
- Replace cumulative-only latency metrics with bounded route histograms and explicit privacy
  suppression state.

## P2 — measured performance leverage

- Cache executable/core/content fingerprints by canonical path, size, modification time, and cache
  schema version. Keep launch-time integrity checks and require an 80% warm-preflight improvement.
- Move high-risk SQL to checked `sqlx` macros with committed offline metadata.
- Split the large room/alpha route modules and Match screen only around measured lifecycle and
  rendering boundaries; preserve transactions and protocol behavior.

## Physical alpha gate

Follow [`alpha/RETROARCH_TEST.md`](alpha/RETROARCH_TEST.md) and [`alpha/LAN_TEST.md`](alpha/LAN_TEST.md).
The decision threshold remains at least 8 verified attempts from 10 independent room attempts,
with failures retained rather than discarded.

Do not expand into friends, broad emulator coverage, or polished discovery until certification and
physical evidence demonstrate a reliable completed-match loop.
