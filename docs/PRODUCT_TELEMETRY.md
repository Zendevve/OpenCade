# Product telemetry

OpenCade product telemetry measures whether players can move from choosing a game to entering its
lobby. It is disabled until a player explicitly opts in, and the choice can be changed from the
Games screen at any time. Telemetry delivery never blocks the match flow.

## Event model

The Rust protocol crate is the source of truth for the closed event contract. The server accepts
only these events:

| Event                 | Meaning                                                 | Allowed blocker checks                                    |
| --------------------- | ------------------------------------------------------- | --------------------------------------------------------- |
| `game_selected`       | A catalog game was selected                             | None                                                      |
| `readiness_completed` | Every required readiness check passed                   | None                                                      |
| `readiness_blocked`   | Lobby entry was attempted with a required check blocked | `desktop`, `control_plane`, `game_runtime`, `native_port` |
| `lobby_entered`       | Lobby presence was created successfully                 | None                                                      |

Each request contains an idempotency UUID, an anonymous tab-session UUID, the game ID, and the
typed blocker list. There is no arbitrary-properties field.

## Privacy and retention

- The server authenticates ingestion but does not store the user ID with an event.
- Usernames, ROM or executable paths, network endpoints, credentials, user-agent strings, free
  text, and error messages are not accepted by the schema.
- The anonymous session identifier lives in `sessionStorage` and therefore does not become a
  durable cross-session identity.
- Raw events older than 90 days are removed by a bounded background maintenance pass. A failed pass
  is logged and retried without delaying event ingestion.
- Ingestion is capped at 60 events per authenticated account per minute.
- The complete summary is suppressed until it contains three selected sessions; blocker dimensions
  with fewer than three events are also omitted.

## Activation metric

`GET /api/v1/telemetry/activation` returns a rolling 30-day summary. Its primary measures are:

- selected-to-ready rate = distinct sessions with `readiness_completed` / distinct sessions with
  `game_selected`;
- selected-to-lobby rate = distinct sessions with `lobby_entered` / distinct sessions with
  `game_selected`;
- readiness blocker events grouped by typed check after small-cohort suppression.

The endpoint requires both a user bearer token and the operator token. Rates are `0` when there are
no selected sessions; the current payload does not yet distinguish privacy suppression from a
genuine zero and must not be used alone for demand decisions.
