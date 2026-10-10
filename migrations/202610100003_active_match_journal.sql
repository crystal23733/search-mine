CREATE TABLE online_active_matches (
    id UUID PRIMARY KEY CHECK (id <> '00000000-0000-0000-0000-000000000000'),
    rules_hash TEXT NOT NULL CHECK (rules_hash ~ '^[0-9a-f]{64}$'),
    owner_id UUID NOT NULL CHECK (owner_id <> '00000000-0000-0000-0000-000000000000'),
    admitted_at TIMESTAMPTZ NOT NULL CHECK (
        admitted_at >= to_timestamp(0) AND
        admitted_at <= to_timestamp((253402300799 - 604800)::double precision) AND
        date_trunc('second', admitted_at) = admitted_at
    )
);
CREATE INDEX online_active_retention ON online_active_matches (admitted_at);

CREATE TABLE online_active_players (
    match_id UUID NOT NULL REFERENCES online_active_matches(id) ON DELETE CASCADE,
    seat SMALLINT NOT NULL CHECK (seat IN (0,1)),
    account_id UUID REFERENCES auth_accounts(id) ON DELETE CASCADE,
    is_bot BOOLEAN NOT NULL,
    PRIMARY KEY (match_id, seat),
    CHECK ((is_bot AND account_id IS NULL) OR (NOT is_bot AND account_id IS NOT NULL))
);
CREATE UNIQUE INDEX online_active_human_seat ON online_active_players (match_id, account_id)
    WHERE account_id IS NOT NULL;
CREATE INDEX online_active_account ON online_active_players (account_id);
