# ADR 0001: Proof-of-Match boundaries

- Status: Accepted for implementation
- Date: 2026-08-23
- Scope: control plane, game-input data plane, and emulator adapter boundary

## Context

OpenFight currently specifies matchmaking/signaling and emulator launch as separate subsystems. A
successful signaling exchange does not itself make an emulator consume peer input, and launching an
emulator with a ROM path does not make it join a synchronized match. Treating WebSocket signaling as
the rollback transport would also couple latency-sensitive input delivery to the control plane.

The MVP needs an executable contract that proves how a room becomes a game before NAT traversal,
relay infrastructure, or additional emulator adapters are implemented.

This design is original and based on public networking concepts. It does not depend on proprietary
source, decompiled behavior, or a proprietary wire format. Any future observation of a reference
system must follow `research/GUARDRAILS.md` before it informs an adapter implementation.

## Decision

OpenFight separates three boundaries.

### Control plane

The Axum server owns authentication, challenges, room membership, room-state transitions, presence,
and exchange of transport candidates. Control messages use the versioned OpenFight envelope.

The control plane never interprets game inputs. It may authorize creation of a relay allocation, but
the allocation belongs to the data plane.

### Data plane

`packages/networking` owns a bidirectional frame transport. Its first implementation is a
deterministic in-memory transport used by tests. The next implementation is direct UDP for LAN
testing. Hole punching, STUN, and relay transports implement the same interface later.

The transport deals in OpenFight input frames, not SDP and not emulator-specific packets. A frame
contains a monotonically increasing frame number, player identity, and opaque input bytes with a
strict size limit.

### Adapter boundary

An adapter receives a validated `MatchDescriptor` before launch:

```rust
pub struct MatchDescriptor {
    pub room_id: String,
    pub game_id: String,
    pub local_user_id: String,
    pub peer_user_id: String,
    pub role: PeerRole,
    pub transport: TransportKind,
    pub local_endpoint: SocketAddr,
    pub peer_endpoint: SocketAddr,
    pub input_delay_frames: u8,
}
```

The descriptor contains coordination data only. Credentials and raw session tokens are never passed
to an emulator process.

An emulator integration is considered netplay-capable only when it can satisfy one of these public,
documented contracts:

1. consume OpenFight frames through an original sidecar/plugin interface;
2. accept documented native netplay endpoint arguments; or
3. expose a permissively licensed public API that an adapter can call without linking incompatible
   code into OpenFight.

Merely launching an emulator is `local_play` capability, not `netplay` capability. The adapter API
must report capabilities so the UI and server never promise a match mode the adapter cannot provide.

## Proof-of-Match acceptance test

The first end-to-end proof uses two authenticated clients and two mock adapters:

1. both users select the same game definition;
2. one user creates a challenge and the other accepts;
3. the server creates a room and authorizes both members;
4. clients exchange transport candidates and derive matching descriptors;
5. two mock adapters start with opposite peer roles;
6. each sends deterministic input frames through the in-memory data plane;
7. both observe the same ordered frame transcript;
8. the room transitions to `PLAYING`, then `FINISHED`;
9. logs correlate the run by `request_id` and `room_id`.

The same test is then run over direct UDP on two LAN hosts. Relay work begins only after LAN success.

## Consequences

- WebSocket remains a control-plane transport; it is not implicitly a game-input relay.
- Existing `signaling.offer/answer/candidate` names may be retained for compatibility, but payloads
  become transport-neutral endpoint candidates rather than WebRTC-only SDP bags.
- `EmulatorAdapter` gains explicit capabilities and match preparation.
- FBNeo remains a candidate adapter, not a proven netplay adapter, until a public or clean-room
  integration path passes the acceptance test.
- Failure to prove a legal real-emulator seam changes the product claim to matchmaking plus safe
  local launch; it does not justify copying a proprietary launcher or protocol.
