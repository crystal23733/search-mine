CREATE TABLE daily_definitions (
    id text PRIMARY KEY CHECK (octet_length(id) BETWEEN 78 AND 82),
    utc_date date NOT NULL CHECK (utc_date BETWEEN DATE '0001-01-01' AND DATE '9999-12-31'),
    seed_version integer NOT NULL CHECK (seed_version BETWEEN 1 AND 65535),
    rules_snapshot jsonb NOT NULL CHECK (jsonb_typeof(rules_snapshot) = 'object'),
    published_at timestamptz NOT NULL CHECK (isfinite(published_at) AND date_trunc('second',published_at)=published_at),
    verification_expires_at timestamptz NOT NULL CHECK (isfinite(verification_expires_at) AND date_trunc('second',verification_expires_at)=verification_expires_at),
    UNIQUE (utc_date, seed_version),
    CHECK (verification_expires_at = (utc_date::timestamp AT TIME ZONE 'UTC') + 604800 * interval '1 second'),
    CHECK (published_at >= (utc_date::timestamp AT TIME ZONE 'UTC') AND published_at < verification_expires_at)
);

CREATE TABLE daily_completions (
    id uuid PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000'),
    account_id uuid NOT NULL REFERENCES auth_accounts(id) ON DELETE CASCADE,
    daily_id text NOT NULL REFERENCES daily_definitions(id),
    attempt_id uuid NOT NULL CHECK (attempt_id <> '00000000-0000-0000-0000-000000000000'),
    replay_digest bytea NOT NULL CHECK (octet_length(replay_digest) = 32),
    opened_safe integer NOT NULL CHECK (opened_safe BETWEEN 1 AND 65535),
    mistakes integer NOT NULL CHECK (mistakes BETWEEN 0 AND 65535),
    replayed_elapsed_ms bigint NOT NULL CHECK (replayed_elapsed_ms BETWEEN 0 AND 4294967295),
    completed_at timestamptz NOT NULL CHECK (isfinite(completed_at) AND completed_at>to_timestamp(0) AND date_trunc('second',completed_at)=completed_at),
    replay_json jsonb CHECK (replay_json IS NULL OR (jsonb_typeof(replay_json) = 'object' AND octet_length(replay_json::text) <= 2097152)),
    replay_expires_at timestamptz NOT NULL CHECK (isfinite(replay_expires_at) AND date_trunc('second',replay_expires_at)=replay_expires_at),
    record_expires_at timestamptz NOT NULL CHECK (isfinite(record_expires_at) AND date_trunc('second',record_expires_at)=record_expires_at),
    UNIQUE (account_id, daily_id),
    UNIQUE (account_id, attempt_id),
    CHECK (EXTRACT(EPOCH FROM replay_expires_at) - EXTRACT(EPOCH FROM completed_at) = 604800),
    CHECK (EXTRACT(EPOCH FROM record_expires_at) - EXTRACT(EPOCH FROM completed_at) = 7776000)
);
CREATE INDEX daily_completions_replay_expiry ON daily_completions(replay_expires_at) WHERE replay_json IS NOT NULL;
CREATE INDEX daily_completions_record_expiry ON daily_completions(record_expires_at);

CREATE FUNCTION reject_daily_definition_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Published daily definitions are immutable' USING ERRCODE = '23514';
END;
$$;
CREATE TRIGGER daily_definition_immutable BEFORE UPDATE ON daily_definitions
    FOR EACH ROW EXECUTE FUNCTION reject_daily_definition_update();

CREATE FUNCTION protect_daily_completion() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(NEW) - 'replay_json') IS DISTINCT FROM (to_jsonb(OLD) - 'replay_json')
       OR (NEW.replay_json IS NOT NULL AND NEW.replay_json IS DISTINCT FROM OLD.replay_json) THEN
        RAISE EXCEPTION 'Official daily completions are immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER daily_completion_immutable BEFORE UPDATE ON daily_completions
    FOR EACH ROW EXECUTE FUNCTION protect_daily_completion();
