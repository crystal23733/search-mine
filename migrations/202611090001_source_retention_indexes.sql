-- Support bounded source cleanup and the associated FK cascade / coverage lookup.
CREATE INDEX product_events_online_retention ON product_events(online_match_id, occurred_at)
    WHERE online_match_id IS NOT NULL;
CREATE INDEX product_events_daily_retention ON product_events(daily_completion_id, occurred_at)
    WHERE daily_completion_id IS NOT NULL;
CREATE INDEX daily_completion_definition_retention ON daily_completions(daily_id);
CREATE INDEX daily_definition_retention ON daily_definitions(verification_expires_at, id);
