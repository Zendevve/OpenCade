#!/usr/bin/env sh
set -eu

: "${DATABASE_URL:?set DATABASE_URL to the PostgreSQL database to back up}"
: "${BACKUP_PATH:?set BACKUP_PATH to a new .dump file}"

if [ -e "$BACKUP_PATH" ]; then
  echo "refusing to overwrite existing backup: $BACKUP_PATH" >&2
  exit 1
fi

umask 077
pg_dump --dbname "$DATABASE_URL" --format custom --no-owner --no-acl --file "$BACKUP_PATH"
pg_restore --list "$BACKUP_PATH" >/dev/null

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$BACKUP_PATH" >"$BACKUP_PATH.sha256"
else
  shasum -a 256 "$BACKUP_PATH" >"$BACKUP_PATH.sha256"
fi
echo "verified backup written to $BACKUP_PATH"
