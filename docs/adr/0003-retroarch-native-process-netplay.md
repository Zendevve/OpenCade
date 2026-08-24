# ADR 0003: Prove play through a user-supplied RetroArch process

- Status: accepted, alpha evidence pending
- Date: 2026-08-24

## Context

ADR 0002 correctly keeps standalone FBNeo netplay blocked because its public command-line surface
does not expose a stable endpoint or session contract. OpenCade nevertheless needs one legally and
technically reviewable path from an authenticated room to a real two-player emulator session.

RetroArch publicly documents loading a Libretro core and content as separate command-line arguments,
host/client netplay modes, a netplay port, and compatible content requirements. This is a process
interface: OpenCade does not link RetroArch or an emulator core and does not redistribute either.

Sources reviewed for this decision:

- https://github.com/libretro/docs/blob/master/docs/guides/cli-intro.md
- https://github.com/libretro/docs/blob/master/docs/guides/netplay-faq.md
- https://github.com/libretro/RetroArch/blob/master/retroarch.cfg
- https://github.com/finalburnneo/FBNeo/issues/2319

## Decision

Add `NetplayMode::NativeProcess` beside `OpenCadeFrames` and
`BlockedNoPublicInterface`. The first native-process adapter targets a separately installed
RetroArch executable plus a separately installed FBNeo Libretro core.

The adapter:

1. detects the executable and core below one configured root;
2. canonicalizes the executable, core, and ROM below their allowlisted roots;
3. hashes all three inputs with SHA-256 and records the declared RetroArch version;
4. starts the host with documented host/port arguments;
5. starts the guest with documented connect/port arguments;
6. passes each argument directly to `Command`, never through a shell; and
7. keeps process ownership in the existing bounded desktop process registry.

For the LAN alpha, RetroArch uses TCP port `55435`. The guest derives the host IP from the already
authenticated OpenCade candidate exchange. OpenCade's deterministic UDP/relay probe remains a
separate readiness test and evidence source; it is not presented as RetroArch's internal data plane.

## Distribution and license boundary

- Users provide RetroArch, the FBNeo core, BIOS files, and ROMs.
- CI artifacts contain only original Apache-2.0 OpenCade code.
- OpenCade does not dynamically or statically link an emulator.
- Paths, binaries, hashes, ROM names, and content are not uploaded by the alpha report.
- This ADR records an engineering boundary, not a legal opinion. Any future redistribution requires
  an independent license review.

## Evidence gates

The adapter is implemented, but real netplay remains unproven until two Windows machines demonstrate:

1. identical executable/core/content fingerprints;
2. one host and one guest launched from the same OpenCade room;
3. a successful RetroArch netplay connection and player-two input;
4. clean process shutdown; and
5. paired, privacy-minimized OpenCade transport reports.

If no stable automated connection-success signal can be obtained from the documented process
surface, retain this adapter as experimental and do not advertise one-click emulator netplay.

## Consequences

- Standalone FBNeo remains local-play only.
- OpenCade becomes capable of orchestrating both its own deterministic frame transport and an
  emulator-owned native netplay transport without conflating them.
- Exact compatibility fingerprints become a prerequisite for playable alpha sessions.
- Direct UDP failure may fall back to OpenCade's authenticated WebSocket relay for the readiness
  probe; that does not transparently relay RetroArch's native TCP session.
