# ADR 0005: Community-alpha orchestration and native tunnel boundary

Status: accepted, 2026-08-25.

## Context

The experimental RetroArch path needed a repeatable two-person loop instead of manual timing and
download-only reports. Reconnects also needed a durable authority for compatibility and launch
state. A TCP-over-WebSocket path is technically possible, but an automated loopback test does not
prove RetroArch behavior across two physical hosts.

## Decision

- The server owns one-use invite redemption, compatibility preflights, monotonic room snapshots,
  and a single persisted `launch_at` barrier.
- Both members must submit matching executable, core, and content fingerprints and confirm their
  native port before the server issues a campaign launch grant.
- Clients resume active rooms from durable snapshots and ignore stale revisions.
- Evidence upload is authenticated, role-bound, size-bounded, privacy-filtered, idempotent for an
  identical report, and rejects a conflicting report for the same room/user/kind.
- Relay tickets carry a cryptographically signed capability. Readiness probes and native TCP
  tunnel traffic cannot share a relay bucket or ticket.
- The native TCP tunnel implementation is shipped as a gated capability, not selected by the
  match coordinator until two-host evidence demonstrates safe RetroArch operation.

## Consequences

The alpha loop is deterministic and recoverable without claiming unearned network compatibility.
Enabling the native tunnel later is a route-policy change backed by evidence, not a new transport
implementation. Campaign summaries read only the newest 1,000 evidence records so the dashboard
cannot grow into an unbounded request.
