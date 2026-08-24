# opencade-relay

TURN-like WebSocket relay fallback for OpenCade.

## Run

```bash
cargo run -p opencade-relay
# env:
#   PORT=8081            # HTTP/health port
#   RELAY_PORT=3478      # TURN port (exposed via health)
#   RUST_LOG=info
#   RELAY_HOST=           # optional
#   OPENCADE_ENV=production  # json logs if production, else pretty
```

## Endpoints

- `GET /health` → `{status:"ok", version:"0.1.0", relay_port}`
- `GET /ready` → `{status:"ok"}` (DB-less)
- `WS /relay?room_id=<id>` → opaque frame relay between peers in same `room_id` bucket; validates `opencade-protocol` Envelope version if present, forwards otherwise.

## Docker

```bash
docker compose up relay
# exposes 8081 (health) and 3478/udp+tcp
```
