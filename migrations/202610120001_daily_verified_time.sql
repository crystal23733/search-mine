ALTER TABLE daily_completions
    ADD COLUMN verified_elapsed_ms bigint NULL
    CHECK (verified_elapsed_ms IS NULL OR verified_elapsed_ms BETWEEN 0 AND 4294967295);
