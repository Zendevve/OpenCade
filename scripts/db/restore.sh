#!/usr/bin/env sh
set -eu

: "${DATABASE_URL:?set DATABASE_URL to the empty PostgreSQL restore target}"
: "${BACKUP_PATH:?set BACKUP_PATH to a verified .dump file}"
: "${CONFIRM_RESTORE:?set CONFIRM_RESTORE=restore-empty-database}"

if [ "$CONFIRM_RESTORE" != "restore-empty-database" ]; then
  echo "restore confirmation did not match" >&2
  exit 1
fi
if [ ! -f "$BACKUP_PATH" ]; then
  echo "backup does not exist: $BACKUP_PATH" >&2
  exit 1
fi

pg_restore --list "$BACKUP_PATH" >/dev/null
table_count=$(psql "$DATABASE_URL" -Atqc \
  "SELECT count(*) FROM pg_catalog.pg_tables WHERE schemaname = 'public'")
if [ "$table_count" != "0" ]; then
  echo "restore target is not empty; refusing to overwrite it" >&2
  exit 1
fi

pg_restore --dbname "$DATABASE_URL" --no-owner --no-acl --exit-on-error "$BACKUP_PATH"
echo "restore completed; run application readiness and smoke tests before promotion"
