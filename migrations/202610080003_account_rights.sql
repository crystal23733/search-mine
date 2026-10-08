ALTER TABLE auth_transactions ADD COLUMN bound_session_hash bytea
    CHECK (bound_session_hash IS NULL OR octet_length(bound_session_hash) = 32);
-- Unbound historical link/reauth transactions cannot acquire authority.
DELETE FROM auth_transactions WHERE intent <> 'login';
ALTER TABLE auth_transactions ADD CONSTRAINT auth_transaction_session_binding
    CHECK ((intent = 'login' AND bound_session_hash IS NULL) OR
           (intent <> 'login' AND bound_session_hash IS NOT NULL));
ALTER TABLE auth_identities ADD CONSTRAINT auth_identity_one_provider UNIQUE(account_id, provider);
ALTER TABLE auth_credentials ADD COLUMN next_check_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE auth_credentials ADD COLUMN check_revision uuid NOT NULL DEFAULT gen_random_uuid();
CREATE TABLE auth_deletion_tombstones (
    account_id uuid PRIMARY KEY,
    deleted_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL CHECK(expires_at = deleted_at + interval '28 days')
);
CREATE TABLE auth_apple_revoke_queue (
    id uuid PRIMARY KEY,
    identity_id uuid NOT NULL UNIQUE,
    encrypted_refresh_token bytea NOT NULL CHECK(octet_length(encrypted_refresh_token) BETWEEN 34 AND 4129),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL CHECK(expires_at = created_at + interval '24 hours'),
    next_attempt_at timestamptz NOT NULL,
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 10000),
    lease_id uuid,
    lease_until timestamptz,
    CHECK ((lease_id IS NULL) = (lease_until IS NULL))
);
CREATE INDEX auth_apple_revoke_due ON auth_apple_revoke_queue(next_attempt_at);
CREATE TABLE auth_apple_notification_receipts (
    jti_digest bytea PRIMARY KEY CHECK(octet_length(jti_digest) = 32),
    expires_at timestamptz NOT NULL
);
