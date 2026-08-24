# ADR 0002: Keep standalone FBNeo netplay blocked

- Status: accepted
- Date: 2026-08-24

## Context

OpenCade needs a documented, automatable emulator seam that accepts a peer/session endpoint and
provides deterministic input/transcript evidence. Local game launch is insufficient.

The public FBNeo material reviewed for this decision says:

- The official README describes netplay as a feature and lists the Libretro port.
- The official UI documentation exposes “Play via Kaillera.”
- The official command-line reference documents game/load/list/resolution options, but no endpoint,
  session, input-stream, or headless netplay contract.
- Open issue #2319 asks whether native standalone netplay is planned and identifies FBNeo-through-
  RetroArch as the currently known path; it does not establish a standalone API.

Sources:

- https://github.com/finalburnneo/FBNeo/blob/master/README.md
- https://github.com/finalburnneo/FBNeo/wiki/Command-Line
- https://github.com/finalburnneo/FBNeo/wiki/menu_game
- https://github.com/finalburnneo/FBNeo/issues/2319

## Decision

The standalone FBNeo adapter remains local-play only and reports
`NetplayReadiness::BlockedNoPublicInterface`. OpenCade will not automate UI dialogs, depend on an
undocumented Kaillera wire contract, or copy behavior from proprietary launchers.

RetroArch/Libretro is a separate future adapter candidate, not an implicit fallback. It requires an
independent process-boundary and license review; OpenCade must not link incompatible emulator code.

## Exit criteria

Change this decision only when one of these clean-room seams exists:

1. FBNeo publishes stable endpoint/session command-line arguments or an API; or
2. an original, independently specified sidecar is accepted upstream and exposes the required seam.

Either path must additionally pass a two-peer deterministic 60-frame transcript test, fail closed on
version mismatch, preserve safe process argument handling, and satisfy the repository license gate.
