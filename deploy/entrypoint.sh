#!/bin/sh
set -eu

invalid() {
    printf '%s\n' 'Invalid runtime secret configuration' >&2
    exit 78
}

# The name is always a literal from the allowlist below; values are never evaluated.
load_secret() {
    secret_name=$1
    secret_file_name=${secret_name}_FILE
    secret_file=$(printenv "$secret_file_name" || true)
    [ -n "$secret_file" ] || invalid
    [ -z "$(printenv "$secret_name" || true)" ] || invalid
    [ -f "$secret_file" ] && [ ! -L "$secret_file" ] && [ -r "$secret_file" ] || invalid
    secret_size=$(wc -c 2>/dev/null < "$secret_file") || invalid
    [ "$secret_size" -gt 0 ] && [ "$secret_size" -le 16384 ] || invalid
    secret_text_size=$(LC_ALL=C tr -d '\000' 2>/dev/null < "$secret_file" | wc -c) || invalid
    [ "$secret_text_size" -eq "$secret_size" ] || invalid
    secret_value=$(cat "$secret_file" 2>/dev/null) || invalid
    [ -n "$secret_value" ] || invalid
    export "$secret_name=$secret_value"
    unset "$secret_file_name"
    unset secret_value secret_file secret_size secret_text_size
}
optional_secret() {
    optional_name=$1
    optional_file=$(printenv "${optional_name}_FILE" || true)
    # Container execution accepts files, never a secret's plaintext config value.
    [ -z "$(printenv "$optional_name" || true)" ] || invalid
    if [ -n "$optional_file" ]; then load_secret "$optional_name"; fi
}

case "${1:-}" in
    serve)
        [ "$#" -eq 1 ] || invalid
        load_secret DATABASE_URL
        load_secret LIAR_AUTH_CSRF_KEY
        load_secret LIAR_AUTH_DIGEST_KEYS
        load_secret LIAR_AUTH_VAULT_KEYS
        optional_secret LIAR_GOOGLE_CLIENT_SECRET
        optional_secret LIAR_KAKAO_CLIENT_SECRET
        optional_secret LIAR_NAVER_CLIENT_SECRET
        exec /app/bin/liar-server
        ;;
    migrate)
        [ "$#" -eq 1 ] || invalid
        load_secret MIGRATION_DATABASE_URL
        exec /app/bin/migrate
        ;;
    metrics)
        shift
        load_secret DATABASE_URL
        exec /app/bin/metrics "$@"
        ;;
    *) invalid ;;
esac
