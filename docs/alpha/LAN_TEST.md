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
4. Confirm both clients reach `connecting` and exchange a correlated signaling message.
5. Run the direct-UDP proof harness on the selected LAN addresses and record connection time,
   frame count, packet loss, and disconnect reason.
6. Transition the test room to `playing`, then `finished`.
7. Export the redacted report from each client and compare `room.id`, `game_id`, and final state.

## Pass criteria

- Both clients agree on the room and users.
- No non-member can mutate or signal into the room.
- The UDP transcript is ordered and identical at both endpoints.
- The match row has `started_at` and `ended_at`.
- Reports contain no session token, password, full ROM path, or emulator binary.

Record each attempt in `MATCH_REPORT_TEMPLATE.md`. Do not schedule relay work until ten LAN attempts
have at least an 80% connection-and-completion rate or show one concentrated, fixable failure.
