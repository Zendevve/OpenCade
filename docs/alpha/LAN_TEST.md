# Proof-of-Match LAN test

Use this script only after the automated suite passes. It tests OpenCade's own control plane and
direct UDP frame transport; it does not claim FBNeo netplay support.

## Before the session

1. For same-LAN tests, set a 32-byte-or-longer `SESSION_SECRET` on the host and run
   `docker compose up --build -d`. For different-NAT tests, deploy the same server behind a
   publicly reachable HTTPS/WSS endpoint; do not expose a developer laptop directly.
2. Confirm `GET /health` and `GET /ready` both return HTTP 200.
3. Point both clients at the LAN server or deployed public control-plane URL with
   `OPENCADE_API_URL`. For LAN, allow TCP 8080 and 8081 through the host firewall.
4. For the different-NAT phase, set `OPENCADE_STUN_SERVER=<numeric-ip>:3478` on both clients. The
   STUN service must implement RFC 8489 Binding. DNS names are deliberately not resolved by the
   alpha client.
5. Download the flat `opencade-windows-alpha` artifact and run `OpenCade-Alpha.ps1 -Mode Doctor`
   on both Windows machines before using `-Mode Launch`. Repository contributors may instead use
   `pnpm -C apps/client tauri dev`.
6. Use fixture-free local ROM scanning only; never attach ROMs or emulator binaries to reports.

## Scenario

1. Register two separate users and select the same game.
2. Keep both lobby screens open until each user is visible.
3. User A sends a challenge; user B accepts it.
4. Confirm both clients reach `connecting`. Each desktop client reserves one UDP socket, optionally
   discovers its reflexive address through STUN on that same socket, then exchanges host/reflexive
   candidates and a nonce through the authenticated WebSocket.
5. The clients send bounded, room/session-bound punch packets to the advertised candidates.
   Unknown sources and mismatched credentials are ignored.
6. Wait for both clients to report `Direct UDP verified`, with 60 received frames and the same
   transcript checksum. The host then transitions the room to `playing` and `finished`.
7. If either side reports a firewall or timeout error, allow the advertised UDP port through the
   firewall and select `Retry LAN probe` on both clients.
8. If the attempt is abandoned, select **Export failure evidence** before leaving the match screen.
   This records only a stable stage/code—never the displayed diagnostic message, endpoints, or
   paths. One or two failure files for the room count as one failed campaign attempt.
9. For a completed attempt, export the redacted report from each client. Copy both reports to one
   machine and verify them:

   ```bash
   opencade-match-verify host-report.json guest-report.json
   ```

   A pass prints JSON with `"verified":true`; a mismatch prints a stable error code to stderr and
   exits non-zero. Reports deliberately omit nonces, endpoints, user identifiers, session material,
   and local paths.

For transport-only diagnosis without the desktop flow, build `opencade-match-probe` and run one
process on each host with complementary arguments:

```bash
cargo run -p opencade-networking --bin opencade-match-probe -- \
  --local 192.168.1.10:42000 --peer 192.168.1.11:42000 \
  --room lan-test --game sfiii3 --local-user host --peer-user guest \
  --role host --session-key shared-test-key --frames 60 --timeout-ms 5000
```

The other host swaps local/peer addresses and users and uses `--role guest` with the same room,
game, session key, frame count, and timeout. The command prints the same canonical, redacted JSON
format as the desktop client. Download the client and Windows tools from the flat,
checksum-verified `opencade-windows-alpha` artifact on a successful `main` CI run, or build them
with:

```bash
cargo build -p opencade-networking --bins --release --locked
```

## Pass criteria

- Both clients agree on the room and users.
- No non-member can mutate or signal into the room.
- The UDP transcript is ordered and identical at both endpoints.
- `opencade-match-verify` accepts the host/guest report pair: same room, game, checksum, finished
  state, opposite roles, direct UDP, and exactly 60 received frames.
- The match row has `started_at` and `ended_at`.
- Successful and failure reports contain no session token, password, endpoint, diagnostic message,
  full ROM path, or emulator binary.

Record each attempt in `MATCH_REPORT_TEMPLATE.md`. Run both direct-UDP and authenticated-relay
attempts: direct UDP measures the preferred path, while relay verifies the production fallback.

## Campaign gate

Run five same-LAN and five different-NAT attempts. Store two successful JSON reports per completed
room, or at least one failure-evidence JSON file per abandoned room, in one directory. Then derive
the gate and compatibility matrix:

```bash
opencade-alpha-summary ./campaign-reports
```

Exit 0 means at least 8 of 10 rooms passed. Exit 1 means the evidence was readable but the gate did
not pass; exit 2 means the input was invalid. A single Binding response only proves `open` (the
mapped address equals the advertised host address) or `mapped`; it does not claim cone/symmetric NAT
classification. That requires RFC 5780 behavior discovery and remains deferred.

The summarizer counts unique room IDs rather than files, separates direct and relay rows, validates
stable failure codes, and rejects a room containing both success and failure evidence.

After the readiness campaign passes, use `RETROARCH_TEST.md` for the separate Proof-of-Play gate.
Readiness-probe success does not by itself prove emulator netplay.
