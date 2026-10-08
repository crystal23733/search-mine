CREATE TABLE online_match_results (
    id UUID PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000'),
    rules_hash TEXT NOT NULL CHECK (rules_hash ~ '^[0-9a-f]{64}$'),
    secret_seed BYTEA CHECK (octet_length(secret_seed) = 8),
    end_elapsed_ms BIGINT NOT NULL CHECK (end_elapsed_ms BETWEEN 0 AND 9007199254740991),
    reason TEXT NOT NULL CHECK (reason IN ('clear','timeout','forfeit','abandoned','server_failure','cancelled')),
    recorded_at TIMESTAMPTZ NOT NULL,
    seed_expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX online_result_retention ON online_match_results (recorded_at);
CREATE INDEX online_seed_retention ON online_match_results (seed_expires_at) WHERE secret_seed IS NOT NULL;
CREATE TABLE online_match_players (
    match_id UUID NOT NULL REFERENCES online_match_results(id) ON DELETE CASCADE,
    seat SMALLINT NOT NULL CHECK (seat IN (0,1)),
    account_id UUID REFERENCES auth_accounts(id) ON DELETE CASCADE,
    is_bot BOOLEAN NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('win','loss','draw','abort','cancelled')),
    opened_safe SMALLINT NOT NULL CHECK (opened_safe BETWEEN 0 AND 256),
    mistakes SMALLINT NOT NULL CHECK (mistakes BETWEEN 0 AND 4096),
    accusations SMALLINT NOT NULL CHECK (accusations BETWEEN 0 AND 4096),
    correct_accusations SMALLINT NOT NULL CHECK (correct_accusations BETWEEN 0 AND accusations),
    PRIMARY KEY (match_id,seat),
    CHECK ((is_bot AND account_id IS NULL) OR (NOT is_bot AND account_id IS NOT NULL))
);
CREATE INDEX online_player_account ON online_match_players (account_id);
