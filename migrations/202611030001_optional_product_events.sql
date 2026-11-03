CREATE TABLE optional_product_events (
    id uuid PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000'),
    owner_session_id uuid NOT NULL REFERENCES auth_sessions(id) ON DELETE CASCADE,
    tab_key uuid NOT NULL CHECK (tab_key <> '00000000-0000-0000-0000-000000000000'),
    consent_revision uuid NOT NULL CHECK (consent_revision <> '00000000-0000-0000-0000-000000000000'),
    purpose text NOT NULL CHECK (purpose IN ('analytics','advertising')),
    occurred_at bigint NOT NULL CHECK (occurred_at BETWEEN 0 AND 253402300799),
    received_at bigint NOT NULL CHECK (received_at BETWEEN 0 AND 253402300799),
    locale text NOT NULL CHECK (locale IN ('en','ko','ja','zh-CN','es','pt-BR','de','fr')),
    action jsonb NOT NULL CHECK (
        jsonb_typeof(action) = 'object' AND action ? 'type'
        AND octet_length(action::text) <= 1024
        AND action->>'type' IN ('landing_view','invite_open','tutorial_start','tutorial_complete','rematch_selected','share_click','ad_request','ad_impression','ad_failed')
        AND action - ARRAY['type','route','run_id','duration_ms','skipped','previous_match_id','date','channel','placement','reason']::text[] = '{}'::jsonb
    ),
    CHECK (received_at >= occurred_at AND received_at - occurred_at < 2592000),
    CHECK (purpose = CASE WHEN action->>'type' IN ('ad_request','ad_impression','ad_failed') THEN 'advertising' ELSE 'analytics' END)
);
CREATE INDEX optional_product_events_raw_expiry ON optional_product_events(occurred_at, id);
CREATE INDEX optional_product_events_owner ON optional_product_events(owner_session_id);
CREATE TRIGGER optional_product_event_immutable BEFORE UPDATE ON optional_product_events
    FOR EACH ROW EXECUTE FUNCTION reject_product_event_update();
