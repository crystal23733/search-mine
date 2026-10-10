ALTER TABLE online_match_results
    ADD COLUMN retention_started_at TIMESTAMPTZ DEFAULT NULL;

UPDATE online_match_results SET retention_started_at = recorded_at;

ALTER TABLE online_match_results
    ADD CONSTRAINT valid_result_retention_anchor CHECK (
        retention_started_at IS NULL OR (
            retention_started_at >= to_timestamp(0) AND
            retention_started_at <= to_timestamp(253402300799::double precision) AND
            retention_started_at <= recorded_at
        )
    );

CREATE INDEX online_result_retention_anchor ON online_match_results
    ((COALESCE(retention_started_at, recorded_at)));
