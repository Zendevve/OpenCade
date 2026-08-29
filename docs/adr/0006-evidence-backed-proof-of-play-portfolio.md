# ADR 0006: Evidence-backed Proof-of-Play portfolio

Status: accepted, 2026-08-28.

## Context

The community alpha still depended on user-supplied ROM content for its first playable proof, lost
momentum after a verified match, and exposed no privacy-safe view of accumulated compatibility
evidence. The native TCP tunnel existed, but promoting it without a physical-host evidence gate
would turn an implementation claim into an unsupported compatibility claim. Controller ambiguity
also remained outside the launch barrier.

## Decision

- Ship an original Apache-2.0 libretro conformance fixture and inert `.ocade` content. It is a
  deterministic two-input test surface, not an emulator or game, and contains no third-party code,
  media, firmware, BIOS, or ROM data.
- Package, checksum, sign, and explicitly install that fixture through the alpha kit. Never download
  or silently replace an existing core or content file.
- Treat a connected controller and the server-assigned player slot as required preflight evidence.
  Host is player 1 and guest is player 2; both must pass before the launch barrier opens.
- Permit the server to select the native TCP tunnel only when an operator enables the policy and the
  same game cohort reaches both the configured physical-attempt minimum and an 80%
  verified-pair rate. Relay configuration is mandatory. The default remains direct LAN and fails
  closed.
- Produce an idempotent post-match receipt only after the canonical two-report verifier passes. The
  receipt creates a fresh one-use, short-lived invite for the same game.
- Publish only aggregate compatibility cohorts containing at least three distinct rooms. Never
  publish raw reports, identities, endpoints, paths, hashes, or invite data.

## Consequences

Contributors can exercise the complete install-to-netplay loop without legally sensitive content.
Route promotion is reversible configuration backed by the same evidence users generate. The public
map communicates demonstrated cohorts rather than a static promise, while the receipt makes a
successful match the start of another match. The fixture proves OpenCade orchestration and libretro
netplay integration only; it does not prove compatibility with FBNeo games or user-supplied content.
