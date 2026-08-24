# Proof-of-Match LAN test

Use this script only after the automated suite passes. It tests OpenCade's own control plane and
direct UDP frame transport; it does not claim FBNeo netplay support.

## Before the session

1. On the host, set a 32-byte-or-longer `SESSION_SECRET` and run
   `docker compose up --build -d`.
2. Confirm `GET /health` and `GET /ready` both return HTTP 200.
3. Point both clients at the host's LAN URL with `VITE_API_URL` and allow TCP 8080 through the host
   firewall.
4. Run `pnpm -C apps/client tauri dev` on both Windows machines.
5. Use fixture-free local ROM scanning only; never attach ROMs or emulator binaries to reports.

## Scenario

1. Register two separate users and select the same game.
2. Keep both lobby screens open until each user is visible.
3. User A sends a challenge; user B accepts it.
4. Confirm both clients reach `connecting`. Each desktop client reserves a UDP port and exchanges a
   nonce-bound `match.endpoint` candidate through the authenticated WebSocket.
5. Wait for both clients to report `Direct UDP verified`, with 60 received frames and the same
   transcript checksum. The host then transitions the room to `playing` and `finished`.
6. If either side reports a firewall or timeout error, allow the advertised UDP port through the
   firewall and select `Retry LAN probe` on both clients.
7. Export the redacted report from each client. Copy both reports to one machine and verify them:

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
format as the desktop client. Download both Windows tools from the `opencade-lan-tools-windows`
artifact on a successful `main` CI run, or build them with:

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
- Reports contain no session token, password, full ROM path, or emulator binary.

Record each attempt in `MATCH_REPORT_TEMPLATE.md`. Do not schedule relay work until ten LAN attempts
have at least an 80% connection-and-completion rate or show one concentrated, fixable failure.
