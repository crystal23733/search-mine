ALTER TABLE online_match_results
    ALTER COLUMN end_elapsed_ms DROP NOT NULL,
    ADD CONSTRAINT unknown_elapsed_requires_failure
        CHECK (end_elapsed_ms IS NOT NULL OR reason = 'server_failure');

ALTER TABLE online_match_players
    ALTER COLUMN opened_safe DROP NOT NULL,
    ALTER COLUMN mistakes DROP NOT NULL,
    ALTER COLUMN accusations DROP NOT NULL,
    ALTER COLUMN correct_accusations DROP NOT NULL,
    ADD CONSTRAINT complete_statistics_or_unknown_abort CHECK (
        (opened_safe IS NOT NULL AND mistakes IS NOT NULL
            AND accusations IS NOT NULL AND correct_accusations IS NOT NULL)
        OR (outcome = 'abort' AND opened_safe IS NULL AND mistakes IS NULL
            AND accusations IS NULL AND correct_accusations IS NULL)
    );
