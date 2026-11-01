-- Bound self-history lookup even when the planner starts from the retained result window.
CREATE INDEX online_player_self_result
    ON online_match_players (account_id, match_id)
    WHERE NOT is_bot;
