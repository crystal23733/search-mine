CREATE INDEX daily_completions_time_ranking
    ON daily_completions(daily_id, verified_elapsed_ms, mistakes, completed_at, id)
    WHERE verified_elapsed_ms IS NOT NULL;
CREATE INDEX daily_completions_completion_ranking
    ON daily_completions(daily_id, mistakes, completed_at, id);
