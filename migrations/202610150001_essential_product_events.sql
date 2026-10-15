CREATE TABLE product_events (
    id uuid PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000'),
    account_id uuid NOT NULL REFERENCES auth_accounts(id) ON DELETE CASCADE,
    purpose text NOT NULL DEFAULT 'essential' CHECK (purpose = 'essential'),
    occurred_at bigint NOT NULL CHECK (occurred_at BETWEEN 0 AND 253402300799),
    received_at bigint NOT NULL CHECK (received_at BETWEEN 0 AND 253402300799),
    online_match_id uuid REFERENCES online_match_results(id) ON DELETE CASCADE,
    daily_completion_id uuid REFERENCES daily_completions(id) ON DELETE CASCADE,
    opponent_type text,
    end_reason text,
    utc_date date,
    mistakes integer,
    trust text,
    CHECK (received_at >= occurred_at AND received_at - occurred_at < 2592000),
    CHECK (
        (online_match_id IS NOT NULL AND daily_completion_id IS NULL
         AND opponent_type IS NOT NULL AND opponent_type IN ('human','bot')
         AND end_reason IS NOT NULL AND end_reason IN ('clear','timeout','forfeit','abandoned','server_failure','cancelled')
         AND utc_date IS NULL AND mistakes IS NULL AND trust IS NULL)
        OR
        (daily_completion_id IS NOT NULL AND online_match_id IS NULL
         AND opponent_type IS NULL AND end_reason IS NULL
         AND utc_date IS NOT NULL AND utc_date BETWEEN DATE '1970-01-01' AND DATE '9999-12-31'
         AND mistakes IS NOT NULL AND mistakes BETWEEN 0 AND 65535
         AND trust IS NOT NULL AND trust IN ('verified_completion','verified_time'))
    )
);
CREATE UNIQUE INDEX product_events_online_source ON product_events(account_id, online_match_id) WHERE online_match_id IS NOT NULL;
CREATE UNIQUE INDEX product_events_daily_source ON product_events(account_id, daily_completion_id) WHERE daily_completion_id IS NOT NULL;
CREATE INDEX product_events_own_page ON product_events(account_id, id);
CREATE INDEX product_events_raw_expiry ON product_events(occurred_at);
CREATE FUNCTION reject_product_event_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'Product events are immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER product_event_immutable BEFORE UPDATE ON product_events
    FOR EACH ROW EXECUTE FUNCTION reject_product_event_update();
