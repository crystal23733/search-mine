#!/bin/sh
set -eu
# Runs only when the dedicated data volume is empty, as the PostgreSQL OS user.
LIAR_DB_OWNER_PASSWORD=$(cat /run/secrets/db_owner_password) || exit 1
LIAR_DB_SERVING_PASSWORD=$(cat /run/secrets/db_serving_password) || exit 1
[ -n "$LIAR_DB_OWNER_PASSWORD" ] && [ -n "$LIAR_DB_SERVING_PASSWORD" ] || exit 1
export LIAR_DB_OWNER_PASSWORD LIAR_DB_SERVING_PASSWORD
if ! psql --no-psqlrc --set=ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" >/dev/null 2>&1 <<'SQL'
\getenv owner_password LIAR_DB_OWNER_PASSWORD
\getenv serving_password LIAR_DB_SERVING_PASSWORD
CREATE ROLE liar_owner LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD :'owner_password';
CREATE ROLE liar_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD :'serving_password';
ALTER DATABASE liar OWNER TO liar_owner;
REVOKE ALL ON DATABASE liar FROM PUBLIC;
GRANT CONNECT, TEMPORARY ON DATABASE liar TO liar_owner;
GRANT CONNECT ON DATABASE liar TO liar_app;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
ALTER SCHEMA public OWNER TO liar_owner;
SQL
then
    printf '%s\n' 'Database role initialization failed' >&2
    exit 1
fi
unset LIAR_DB_OWNER_PASSWORD LIAR_DB_SERVING_PASSWORD
