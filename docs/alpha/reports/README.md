# LAN Alpha Reports — Manual Gate

This directory is intentionally empty in the repository. Each physical LAN attempt produces one redacted JSON report via:

- Desktop: `Export Report` in Match screen (redacted `room_id`, `game_id`, `local/peer_user_id`, `role`, `transport:direct_udp`, `frames_received:60`, `transcript_checksum`, `room_state:FINISHED`)
- CLI probe: `cargo run -p opencade-networking --bin opencade-match-probe -- --local <ip:port> --peer <ip:port> --room <uuid> --game sfiii3 --local-user host --peer-user guest --role host --session-key <key> --frames 60 --timeout-ms 5000` (prints JSON)

Local 10/10 proof (single box, loopback) is automated:

```
for i in 1..10; do cargo test -p opencade-networking --test two_process_probe -- --nocapture; done
```

Result 10/10 on 2026-08-24 (see CI).

Physical LAN requires 2× Windows 10/11 on same subnet, `docker compose up --build -d`, `VITE_API_URL=http://<host-lan-ip>:8080`, firewall TCP 8080 + UDP probe ports. Follow `docs/alpha/LAN_TEST.md` and save each attempt as `report-01.json` … `report-10.json` then `jq` check:

```bash
ls docs/alpha/reports/*.json | wc -l  # expect 10
jq -e '.room_id and .game_id=="sfiii3" and .frames_received==60 and .transcript_checksum' docs/alpha/reports/*.json
# per pair checksums must match
```

Pass is ≥8/10 COMPLETED with ordered 60-frame transcripts. Do not fabricate reports — they must come from real 2-machine runs. Until then, this gate is `HALT: physical LAN not available in this single-box env — local 10/10 + tooling ready`.
