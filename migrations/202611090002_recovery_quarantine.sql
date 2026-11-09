-- This table's DATA must never be copied by the managed backup command.
CREATE TABLE service_recovery_control (
    singleton boolean PRIMARY KEY CHECK(singleton),
    state text NOT NULL CHECK(state IN ('active','quarantined')),
    database_oid bigint NOT NULL CHECK(database_oid>0),
    schema_oid bigint NOT NULL CHECK(schema_oid>0),
    recovery_id uuid NOT NULL CHECK(recovery_id<>'00000000-0000-0000-0000-000000000000'),
    updated_at bigint NOT NULL CHECK(updated_at BETWEEN 0 AND 253402300799)
);
-- An empty/missing control row is blocked, including after an old archive is migrated.
CREATE TABLE auth_identity_tombstones (
    identity_id uuid PRIMARY KEY,
    deleted_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL CHECK(expires_at=deleted_at+interval '2419200 seconds')
);
CREATE INDEX auth_identity_tombstone_expiry ON auth_identity_tombstones(expires_at);
