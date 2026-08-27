# Production operations runbook

OpenCade separates schema migration from application startup. Production configuration fails closed unless the session, relay, and operator secrets are at least 32 characters and the advertised relay/STUN endpoints are public.

## Deploy and rollback

1. Back up the database with `DATABASE_URL=... BACKUP_PATH=... scripts/db/backup.sh`.
2. Pull immutable images for the intended release tag; never deploy `latest`.
3. Run `opencade-server --migrate` as a one-shot job. Migrations must complete before new server replicas start.
4. Start relay, then server. Check `/health`, `/ready`, and the authenticated `/metrics` endpoint.
5. Exercise register/login, create/accept/cancel room, and a relay-ticket connection.
6. Roll application images back to the prior tag when the schema remains backward compatible. For an incompatible schema, stop writes and restore into a new, empty database using the procedure below; do not mutate the failed database in place.

Docker Compose implements steps 3–4 with the `migrate` service and `service_completed_successfully` dependency.

## Backup and restore drill

Backups are PostgreSQL custom archives, created with mode `0600`, validated with `pg_restore --list`, and accompanied by SHA-256 checksums. Store them encrypted outside the application host.

```bash
DATABASE_URL=postgres://... \
BACKUP_PATH=/secure/opencade-$(date +%Y%m%d).dump \
scripts/db/backup.sh
```

Restore only into an empty database. The restore script fails closed when public tables already exist.

```bash
DATABASE_URL=postgres://.../opencade_restore \
BACKUP_PATH=/secure/opencade-20260826.dump \
CONFIRM_RESTORE=restore-empty-database \
scripts/db/restore.sh
```

Run this drill before the first public release and quarterly afterward. Record archive checksum, start/end time, restored row counts, and smoke-test result.

## Monitoring and incidents

Scrape `GET /metrics` with `x-operator-token: $OPERATOR_TOKEN`. Alert on 5xx responses, readiness failure, server/relay restart loops, active WebSocket collapse, and match-attempt deadline expirations. Campaign and activation aggregates require both a user bearer token and the operator token.

Use `scripts/ops/decision-summary.sh` with `OPENCADE_API_URL`, `SESSION_TOKEN`, and `OPERATOR_TOKEN` to retrieve the bounded campaign cohort and privacy-thresholded activation funnel. Treat a low cohort as insufficient evidence, not as zero demand; launch conversion is reported separately from readiness and lobby conversion.

The room event journal and `match_attempts` table are the incident timeline. Raw evidence is canonicalized and capped at 64 KiB; do not request users to share session tokens, ROM paths, or endpoint addresses.

Rotate a compromised secret independently, revoke sessions when the session secret is affected, restart the corresponding services, and document the time window. Never log secret values.
