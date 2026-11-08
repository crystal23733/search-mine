-- Apply as the migration owner after the reviewed migrations. No DDL privilege for serving.
BEGIN;
REVOKE ALL ON SCHEMA public FROM liar_app;
GRANT USAGE ON SCHEMA public TO liar_app;
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM liar_app;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM liar_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON
    auth_accounts, auth_identities, auth_credentials, auth_sessions, auth_transactions,
    auth_deletion_tombstones, auth_apple_revoke_queue, auth_apple_notification_receipts,
    online_match_results, online_match_players, online_active_matches, online_active_players,
    daily_definitions, daily_completions, session_consents
TO liar_app;
GRANT SELECT, INSERT, DELETE ON product_events, optional_product_events TO liar_app;
GRANT UPDATE(id) ON product_events, optional_product_events TO liar_app;
GRANT SELECT, INSERT ON metric_collection TO liar_app;
GRANT UPDATE(coverage_since) ON metric_collection TO liar_app;
GRANT SELECT, INSERT, DELETE ON product_metric_aggregates TO liar_app;
GRANT UPDATE(computed_at, observed_through, complete, participants, repeat_accounts,
    human_participations, bot_participations, sample_size, retained)
ON product_metric_aggregates TO liar_app;
COMMIT;
