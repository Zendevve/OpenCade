# LAN Alpha Reports — Manual Gate

This directory is intentionally empty in the repository. Each physical LAN attempt produces a pair
of canonical schema-v1 redacted JSON reports via:

- Desktop: `Export report` after the room reaches `finished`.
- Desktop: `Export failure evidence` when an attempt is abandoned.
- CLI probe: `opencade-match-probe --local <ip:port> --peer <ip:port> --room <uuid> --game sfiii3 --local-user host --peer-user guest --role host --session-key <key> --frames 60 --timeout-ms 5000`.

All producers omit identities, endpoints, nonces, session material, credentials, diagnostic
messages, and local paths.

Local 10/10 proof (single box, loopback) is automated:

```
for i in 1..10; do cargo test -p opencade-networking --test two_process_probe -- --nocapture; done
```

Result 10/10 on 2026-08-24 (see CI).

Physical LAN requires 2× Windows 10/11 on the same subnet, `docker compose up --build -d`,
`OPENCADE_API_URL=http://<host-lan-ip>:8080`, a client-reachable `RELAY_URL`, firewall TCP
8080/8081 + UDP probe ports. Run the alpha-kit doctor, follow `docs/alpha/LAN_TEST.md`, save each
successful attempt as `attempt-01-host.json` and `attempt-01-guest.json`, or an abandoned attempt as
`attempt-01-failure.json`, then summarize the directory:

```bash
opencade-alpha-summary .
```

The summary pairs reports by room ID, verifies each pair, and emits a compatibility matrix by game,
platform, observed mapping, selected transport, and optional RetroArch/core/content fingerprints.
Pass is at least 8 verified attempts out of 10. Do not fabricate reports—they must come from real
two-machine runs. Until then, this gate is `HALT: physical LAN not available in this single-box
environment—local 10/10 + evidence tooling ready`.
