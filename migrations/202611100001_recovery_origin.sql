-- Stable source identity is included in managed dumps; it never grants activation.
CREATE TABLE service_recovery_origin (
    singleton boolean PRIMARY KEY CHECK(singleton),
    source_id uuid NOT NULL CHECK(source_id<>'00000000-0000-0000-0000-000000000000')
);
-- Only an existing active deployment may establish its initial source identity.
INSERT INTO service_recovery_origin(singleton,source_id)
SELECT true,recovery_id FROM service_recovery_control
WHERE singleton AND state='active'
  AND database_oid=(SELECT oid::bigint FROM pg_database WHERE datname=current_database())
  AND schema_oid=current_schema()::regnamespace::oid::bigint;
