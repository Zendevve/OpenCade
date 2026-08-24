# ADR 0004: Authorize and observe the native match lifecycle

- Status: accepted
- Date: 2026-08-24

## Context

A successful readiness probe is not proof that an emulator started or gameplay occurred. OpenCade's
probe and relay carry OpenCade UDP frames; RetroArch owns a separate TCP netplay process. Advancing a
room from `CONNECTING` to `PLAYING` or `FINISHED` from probe results therefore produced false state.

## Decision

- Alpha rooms contain exactly two participants.
- The server derives room, game, users, and role, then issues a hashed, 90-second, one-use launch
  grant bound to numeric endpoints and input delay.
- The Tauri core consumes the grant directly and constructs the native descriptor from the server
  response. The webview cannot provide room identity, peer identity, role, or game to the launcher.
- Each participant confirms launch only after process spawn. The room becomes `PLAYING` only after
  both launches are recorded.
- Tauri owns, drains, reaps, and reports the child process. A room becomes `FINISHED` only after both
  recorded native processes exit; an exit while waiting for the peer cancels the room.
- Only a verified same-LAN host candidate can launch RetroArch in this alpha. UDP reflexive and
  authenticated-relay probes remain readiness evidence and are explicitly non-playable because
  they do not carry RetroArch's TCP netplay traffic.
- Playable evidence verification can require matching, syntactically valid executable, core, and
  content fingerprints.

## Consequences

Room state is now evidence-backed and retry-safe at the process boundary. Relay and NAT traversal
remain honest capability gaps instead of being presented as gameplay. A future native transport
bridge must add an explicit route capability before those paths can enable launch.
