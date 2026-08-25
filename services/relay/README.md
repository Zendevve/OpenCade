# opencade-relay

Authenticated WebSocket readiness-probe relay fallback for OpenCade. It is not a TURN server and
does not transparently relay RetroArch's native netplay connection.

## Run

```bash
cargo run -p opencade-relay
# env:
#   PORT=8081            # HTTP/health port
#   RUST_LOG=info
#   RELAY_AUTH_SECRET=    # required; same 32-byte-or-longer secret as opencade-server
#   OPENCADE_ENV=production  # json logs if production, else pretty
```

## Endpoints

- `GET /health` → `{status:"ok", version:"0.1.0"}`
- `GET /ready` → `{status:"ok"}` (DB-less)
- `WS /relay?room_id=...&user_id=...&expires_at=...&signature=...` → accepts a short-lived ticket
  issued by `POST /api/v1/rooms/:id/relay-ticket`; fixes the socket to that room, permits at most two
  users, and forwards bounded binary frames or valid room-scoped protocol envelopes.

## Docker

```bash
docker compose up relay
# exposes 8081 for health and authenticated WebSocket relay traffic
```
