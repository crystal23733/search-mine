#!/bin/sh
set -eu
PGPASSWORD=$(cat /run/secrets/db_owner_password) || exit 1
[ -n "$PGPASSWORD" ] || exit 1
export PGPASSWORD
export PGHOST=postgres PGPORT=5432 PGUSER=liar_owner PGDATABASE=liar
if ! psql --no-psqlrc --set=ON_ERROR_STOP=1 --file=/scripts/serving-grants.sql >/dev/null 2>&1; then
    printf '%s\n' 'Database runtime grant failed' >&2
    exit 1
fi
