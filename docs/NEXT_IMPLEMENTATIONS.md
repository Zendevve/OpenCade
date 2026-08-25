# OpenCade Next Implementations

> Reviewed: 2026-08-23
> Scope: `README.md`, `AGENTS.md`, `docs/ARCHITECTURE.md`, clean-room guardrails,
> workspace code, migrations, CI, and local build/test results.

> **Implementation update (2026-08-23):** the automated Proof-of-Match plan below has been
> implemented. See `docs/IMPLEMENTATION_STATUS.md` for the verified result and the remaining
> real-hardware/NAT validation work. The original review is retained as the decision record.

> **Traversal update (2026-08-24):** RFC 8489 discovery, bounded nonce-bound hole punching, and
> automated 8-of-10 campaign aggregation are now implemented. Physical Windows evidence remains
> the gate; cone/symmetric classification and relay fallback remain deferred. The sequencing below
> is retained as the original pre-evidence plan.

> **Proof-of-Play update (2026-08-24):** ADR 0003, the user-supplied RetroArch/FBNeo-core
> native-process adapter, compatibility fingerprints, signed relay tickets, bounded relay fallback,
> runtime-configurable Windows alpha executable, and automated tests are implemented. The next gate
> is execution, not another subsystem: run the physical two-Windows campaign in
> `docs/alpha/RETROARCH_TEST.md` and `docs/alpha/LAN_TEST.md` without fabricating results.

> **Campaign-kit update (2026-08-24):** CI now produces a flat, checksummed Windows alpha kit. Its
> shared PowerShell entrypoint packages and verifies the artifact in CI, then verifies API
> health/readiness, the user-supplied RetroArch layout, optional STUN syntax, and runtime launch
> configuration on tester machines. The remaining gate is still physical execution and paired
> report collection.

> **Evidence-integrity update (2026-08-24):** abandoned rooms now export a distinct redacted
> failure-evidence record with an enum stage and validated stable code. Campaign aggregation counts
> unique successful or failed room IDs, separates direct/relay rows, and rejects conflicting
> success/failure evidence. This removes survivorship bias from the 8-of-10 gate.

> **Native-lifecycle hardening (2026-08-24):** opportunities scoring above 20 are implemented in
> ADR 0004: server-derived one-use launch authorization, two-participant launch/exit state,
> supervised processes, a deterministic client coordinator, strict compatibility evidence,
> authentication throttling/timing equalization, secure desktop token storage, and a narrowed CSP.
> Relay and reflexive UDP probes are deliberately readiness-only for the RetroArch TCP alpha.

## Recommendation: build the executable Proof of Match

The next implementation should be one narrow, deterministic vertical slice:

> Two authenticated users select one game, challenge and accept, establish a transport session,
> start two mock emulator adapters with the same match descriptor, exchange deterministic input
> frames, transition the room to `FINISHED`, and preserve a correlated log for the whole run.

This is the keystone because it forces the protocol, room state machine, authentication, WebSocket
hub, client orchestration, networking boundary, and emulator adapter boundary to become one real
system. It also tests the largest unresolved assumption: how an established peer session reaches
the emulator's actual netplay/input loop.

Do not implement TURN, friends, reports, replay, multiple adapters, or polished route coverage
before this slice works on a LAN with a mock adapter and has a documented path to a real adapter.

## What is actually implemented

The workspace has a useful M0/M1 foundation, but it is not yet an executable M1 product.

| Area             | Evidence in code                                                                                                                         | Assessment                                                                                       |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Build baseline   | Rust formatting passes; Clippy passes; 39 Rust tests pass; TS packages build; 17 Vitest tests pass; client builds; Compose config parses | Healthy foundation                                                                               |
| Protocol         | Rust envelope and payloads, TS mirror, generated TS files                                                                                | Useful, but the handwritten TS and generated types can drift                                     |
| Database         | Users/sessions and game/room migrations with five seeded games                                                                           | Schema exists, but no migration path is run by the server/container                              |
| Server           | More complete handlers exist under `src/routes/` and `src/ws.rs`                                                                         | Runtime bypasses them and routes duplicate stubs declared in `main.rs`                           |
| Auth             | Argon2 and hashed opaque-session implementation exists                                                                                   | Not wired; runtime returns `stub-jwt-token`; no `AuthUser` extractor                             |
| Rooms            | Handler code and protocol states exist                                                                                                   | Queries do not match migrations (`slug`, UUID game ids, `host_id`, `guest_id`, lowercase states) |
| WebSocket        | Envelope validation exists in `src/ws.rs`                                                                                                | Not wired; runtime endpoint is an unauthenticated echo service                                   |
| Client           | Vite/Tauri shell and a placeholder Games component                                                                                       | `App.tsx` does not route to Games; no API client, WS client, auth, or session flow               |
| Emulator         | Minimal trait and TOML game loader                                                                                                       | FBNeo adapter is empty; no safe process runtime; no match/network context reaches the adapter    |
| Networking       | Placeholder crate                                                                                                                        | No direct transport, hole punching, latency, or relay implementation                             |
| Desktop security | Tauri allowlist exists                                                                                                                   | `shell-open` is enabled and CSP is null, contradicting the repository guardrails                 |
| Relay/docs       | README and architecture describe a relay service and broad route tree                                                                    | `services/relay`, most routes, protocol docs, and integration tests do not exist                 |

### The critical architecture gap

The design currently stops at two disconnected facts:

1. `packages/networking` should establish direct UDP or a relay path.
2. `EmulatorAdapter::launch` starts a process with a ROM path.

There is no contract for passing peer endpoints, role, session identity, input delay, transport
handles, or synchronized input frames into the emulator. WebRTC-style SDP signaling does not by
itself connect a native arcade emulator to rollback netplay, and a WebSocket signaling relay is not
automatically a game-input relay.

This must be resolved before NAT traversal or UI breadth. Otherwise the project can produce a
working lobby and launcher without producing netplay.

## Quick opportunities scan

### Existing assets

- **Product:** coherent clean-room architecture and a compiling Rust/TypeScript monorepo.
- **Content:** authoritative architecture, contribution rules, guardrails, and an active Discord
  invitation.
- **Audience/distribution:** the Discord community is the only evidenced channel; its size and
  activity are not measured in the repository.
- **Technology:** Axum/Postgres skeleton, versioned protocol, Tauri shell, game definitions, adapter
  trait, Docker and CI.
- **Data:** five game definitions and no product telemetry or completed-match data.
- **Revenue/pricing:** none evidenced; pricing work is premature for an open-source pre-alpha.

### Top three combinations

#### 1. Proof-of-Match contract — 23/25

- **Combination:** protocol + room/auth handlers + client shell + adapter interface, connected by
  one executable scenario.
- **Tier:** T1, combinatorial.
- **Effort:** 4–6 focused weeks for one developer.
- **Impact:** converts many independent skeletons into one falsifiable product claim.
- **First step:** write the control-plane/data-plane and netplay-adapter ADR described in Gate 0.

#### 2. Runtime truth consolidation — 21/25

- **Combination:** existing production-intent modules + migrations + current test suite, connected
  through one `build_app` composition root.
- **Tier:** T1, combinatorial.
- **Effort:** 3–5 days.
- **Impact:** replaces false-positive stub behavior with testable failures and makes later work land
  on the actual runtime.
- **First step:** delete the duplicate handlers/types in `main.rs` and route through the modules.

#### 3. Community alpha loop — 17/25

- **Combination:** Discord + deterministic two-peer harness + correlated diagnostics.
- **Tier:** T3, channel leverage.
- **Effort:** 2–3 days after Proof of Match.
- **Impact:** turns community interest into reproducible compatibility and connectivity evidence.
- **First step:** publish a two-machine alpha script and a structured match report template.

### Bottleneck flip

- **Current bottleneck:** no executable path proves that matchmaking becomes a synchronized game.
- **If removed:** every subsequent feature can be judged by completed-match rate rather than files,
  routes, or milestone labels.
- **Removal:** make `proof_of_match` the top-level integration test and alpha demo.

### Pricing test

Not applicable. OpenCade has no evidenced monetization or active usage baseline. The useful test is
adoption: can 20 community testers complete 10 cross-machine matches with at least 80% successful
connection-and-launch attempts? Monetization should be reconsidered only after that signal exists.

## Strategic opportunity scan

### Grove: forces

- **Technology:** Rust/Tauri and public networking standards make a small, self-hostable control
  plane feasible without a large operations team.
- **Customer:** fighting-game communities value low-latency matches and durable community ownership,
  but repository evidence does not yet quantify demand.
- **Competition:** proprietary incumbent behavior creates room for transparency and self-hosting;
  competitor timing is not assessed here and should not be invented from code.
- **Complementors:** external emulators, public rollback literature, Docker, and community-operated
  servers reduce how much OpenCade must own.
- **Constraint:** emulator integration is the underpriced force. Matchmaking is conventional;
  producing a legal, maintainable netplay seam is the hard differentiator.

### Thiel: the secret

Most netplay-platform roadmaps treat lobby, auth, signaling, NAT, and emulator launch as separable
milestones. The non-consensus truth is that the product is the seam between them. A deterministic,
contract-tested match kernel is more valuable than broad but disconnected feature coverage.

### Yeo: the keystone

`proof_of_match` integration contract
→ one canonical protocol
→ one enforceable room state machine
→ a real transport-to-adapter boundary
→ reproducible diagnostics
→ community alpha evidence
→ safe expansion to NAT/relay and additional adapters.

### Jobs: category definition

“OpenCade is the self-hostable, clean-room arcade netplay stack whose entire match lifecycle is
executable and auditable.”

This makes feature-count comparison less important. To make the statement true, say **no** to route
breadth and emulator breadth until one complete match is reproducible.

### Naval: leverage audit

- **Permissionless:** code, documentation, CI, and community distribution require no enterprise
  sales or proprietary service dependency.
- **Specific knowledge:** the valuable intersection is Rust systems work, desktop security,
  networking, emulator process integration, and clean-room discipline.
- **Compounding asset:** each adapter and transport can reuse the same match contract and test kit.
- **Leverage warning:** maintaining a custom TURN-like relay or emulator fork too early creates
  ongoing labor. Prefer public protocols and externally managed emulator binaries behind contracts.

### Bezos: regret test

The larger regret is spending months completing peripheral milestones before discovering that the
chosen emulator cannot consume the negotiated session. The feasibility spike is a reversible,
low-cost decision; take it now.

### Quantitative sizing and affordable loss

Repository evidence is insufficient for honest TAM, revenue EV, or competitor-timing estimates.
Do not fabricate them.

- **Reachable 90-day validation segment:** the existing Discord community.
- **SOM proxy:** 20 alpha participants and 10 completed cross-machine matches.
- **Product success threshold:** ≥80% connection-and-launch success on the supported LAN test matrix.
- **Engineering budget:** six focused weeks, approximately 120–180 hours.
- **Cash affordable loss:** local hardware plus less than US$100 of hosted test infrastructure.
- **Optionality:** real FBNeo support, other adapters, community relay nodes, contributor test kits,
  and eventually managed hosting.

### Opportunity score

| Dimension               |     Score | Reason                                                                              |
| ----------------------- | --------: | ----------------------------------------------------------------------------------- |
| 10X force               |       3/5 | Strong technical leverage; demand and market timing are unmeasured                  |
| Secret quality          |       4/5 | The transport-to-emulator seam is specific and currently missing                    |
| Keystone leverage       |       5/5 | One contract forces six subsystems to converge                                      |
| Lollapalooza            |       3/5 | Code, standards, community, and clean-room positioning align; usage proof is absent |
| Category ownership      |       3/5 | Auditable self-hosted netplay is distinctive but externally unvalidated             |
| Permissionless leverage |       5/5 | Built primarily with code, docs, CI, and community                                  |
| Regret asymmetry        |       4/5 | Cheap to test now; expensive to discover late                                       |
| Timing window           |       3/5 | Open, but no evidence that it is closing within 6–18 months                         |
| **Total**               | **30/40** | Strong asymmetric implementation bet; start this week                               |

## Six-week implementation plan

### Gate 0 — prove the netplay seam (days 1–3)

Write an ADR, using public specifications and the clean-room process, that separates:

- **Control plane:** auth, presence, challenge, room state, endpoint negotiation.
- **Data plane:** latency-sensitive game inputs carried direct or through an explicit relay.
- **Adapter boundary:** how an established `MatchDescriptor` reaches the emulator.

The initial contract should carry at least:

```rust
struct MatchDescriptor {
    room_id: String,
    game_id: String,
    local_user_id: String,
    peer_user_id: String,
    role: PeerRole,
    transport: TransportKind,
    local_endpoint: SocketAddr,
    peer_endpoint: SocketAddr,
    input_delay_frames: u8,
}
```

The exact API may change in the ADR, but the information cannot disappear. Implement a
`MockAdapter` and deterministic two-peer input exchange before claiming FBNeo support.

**Exit gate:** a documented, legally usable path exists for a real emulator to consume match
parameters. If FBNeo cannot do so through a public interface or clean-room-designed bridge, change
the product claim or select a different externally managed emulator before continuing.

### Phase 1 — make the runtime truthful (week 1)

1. Replace `main.rs` duplicates with a small composition root and a reusable `build_app(state)`.
2. Wire `config`, `error`, `state`, `routes`, and `ws` modules into the actual router.
3. Choose the canonical schema already documented: text game ids, `host_user_id`, `room_members`,
   and one uppercase-at-rest/lowercase-on-wire state representation.
4. Update all SQL to that schema; never turn database errors into successful seeded/stub responses.
5. Add `--migrate` and make the container run migrations before serving.
6. Make `/health` liveness-only and `/ready` verify Postgres without returning raw DB errors.
7. Remove Tauri `shell-open`, set a restrictive CSP, and enforce production session-secret checks.
8. Add CI checks for `pnpm -r test`, typecheck, `docker compose config`, and Rust 1.98 MSRV.

**Exit:** a clean database migrates, five real games are returned, bad DB state fails visibly, and
router integration tests exercise the same app that `main` serves.

### Phase 2 — two-user control plane (weeks 2–3)

1. Make Rust protocol types authoritative; generate TS bindings in CI and fail on diff.
2. Split signaling into message-specific payloads instead of one optional-field bag.
3. Add `AuthUser` extraction from hashed opaque sessions; protect REST and WebSocket endpoints.
4. Key the WS hub by authenticated user id, use bounded queues, and reject duplicate/stale sockets
   deterministically.
5. Implement a pure, unit-tested room transition function and persist transitions transactionally.
6. Add durable or explicitly in-memory challenges with create/accept/decline/cancel ownership rules.
7. Relay session negotiation only to authenticated room members; cap payload size and rate.
8. Preserve incoming `request_id` in acknowledgements and correlated errors.
9. Add a Postgres-backed integration test with two users:
   register → login → challenge → accept → negotiate → `CONNECTING` → finish/cancel.

**Exit:** no runtime stubs, no unauthenticated room mutation, and one automated two-user control-plane
test passes against Postgres.

### Phase 3 — minimum client path (week 4)

Implement only the routes needed by Proof of Match:

1. Login/register.
2. Games list with local availability status.
3. One game lobby with presence and challenge action.
4. Incoming challenge acceptance.
5. Connecting/playing/finished state screen.

Add a typed REST client, authenticated WS client, reconnect backoff, request correlation, and minimal
session state. Do not build Friends, Servers, or a design system yet.

**Exit:** two client instances complete the control-plane flow without manual HTTP/WS tools.

### Phase 4 — safe native execution and mock match (week 5)

1. Expand `emulator-sdk` with `DetectedEmulator`, `ValidationReport`, `MatchDescriptor`, and a
   testable process-launch abstraction.
2. Keep paths as `PathBuf`/`OsString`; canonicalize the binary and ROM; verify both are below
   configured roots; never use a shell.
3. Implement Tauri commands for scan, validate, launch, stop, and diagnostics.
4. Implement the FBNeo adapter only to the extent proven by Gate 0.
5. Use tiny fixtures and a mock executable in automated tests—never ROMs or emulator binaries.
6. Connect two mock adapters through the data-plane contract and exchange deterministic frames.

**Exit:** a path containing spaces launches safely in tests, traversal is rejected, and two mock
adapters complete a deterministic session from the same room descriptor.

### Phase 5 — LAN alpha and evidence loop (week 6)

1. Run on two Windows machines on the same LAN before implementing STUN or relay.
2. Add structured correlation fields: `request_id`, `room_id`, `user_id`, `adapter`, transport,
   transition, and failure code—never tokens or full user paths.
3. Add a one-click redacted match report/export.
4. Publish one supported-game alpha script to Discord and collect structured outcomes.
5. Record success rate, setup time, connection time, launch failures, and disconnect reasons.

**Exit:** 10 completed cross-machine matches or enough structured failures to invalidate a core
assumption. Only then schedule hole punching, STUN, and relay fallback.

## Explicitly deferred

- `services/relay` and TURN-like infrastructure.
- Symmetric-NAT support and packet-loss tuning.
- Friends, general chat, reports, bans, spectating, replay, rankings, and matchmaking queues.
- Flycast/Snes9x or a broad game catalog.
- Installer signing, auto-update, production metrics, and UI redesign.
- Monetization or hosted-service pricing.

These are not rejected. They are downstream of a completed match and should be prioritized using
observed alpha failure rates.

## Pre-mortem: it is six months later and this failed

| Cause of failure                                                                      | Mitigation now                                                                                    |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| FBNeo can launch a ROM but cannot consume OpenCade's session/input transport          | Gate 0 before further product work; use a mock adapter and require a documented real-adapter path |
| Stub success responses hide schema and state-machine failures                         | Remove fallbacks; test the composed router against migrated Postgres                              |
| WebSocket signaling is mistaken for a suitable rollback data plane                    | Separate control/data contracts and measure the data plane independently                          |
| Protocol copies drift across Rust, generated TS, and handwritten TS                   | One schema/type source plus a generation-diff CI gate                                             |
| Scope expands into relay, social features, and extra emulators before one match works | Enforce the deferred list and the phase exit gates                                                |

## Kill and re-scope criteria

- **End of day 3:** no credible real-emulator netplay seam → stop using “rollback netplay” as an
  implemented promise and resolve emulator strategy first.
- **End of week 1:** the composed server cannot migrate and serve real data → stop feature work and
  finish runtime consolidation.
- **End of week 3:** two authenticated clients cannot complete challenge and session negotiation in
  an automated test → do not start native integration.
- **End of week 5:** mock adapters cannot complete deterministic frame exchange → do not start NAT
  traversal or relay work.
- **End of week 6:** fewer than 10 completed LAN matches, or below 80% connection-and-launch success
  without a concentrated fixable cause → re-scope the architecture before adding features.

## Execution commitment

- **Wish:** a repeatable two-player match from login to clean shutdown.
- **Outcome:** future adapters and transports plug into one audited contract.
- **Obstacle:** the repository rewards visible breadth—new crates, routes, and milestones—before
  proving the hidden emulator/netplay seam.
- **Plan:** if a task does not increase the probability or observability of `proof_of_match`, defer it
  until the six-week gate is passed.

**Ship this week:** runtime truth consolidation plus the netplay-seam ADR and mock contract.

**Build this quarter:** the executable Proof of Match and a 20-person community alpha.
**Kill criteria:** no real-emulator seam by day 3, no automated two-user control plane by week 3, or
no deterministic mock data-plane match by week 5.
