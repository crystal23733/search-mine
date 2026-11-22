ALTER TABLE online_active_matches
    ADD COLUMN room_id UUID CHECK (room_id <> '00000000-0000-0000-0000-000000000000'),
    ADD COLUMN participant_count SMALLINT NOT NULL DEFAULT 2 CHECK (participant_count BETWEEN 2 AND 4),
    ADD COLUMN started_at TIMESTAMPTZ,
    ADD CHECK (room_id IS NOT NULL OR (participant_count = 2 AND started_at IS NULL)),
    ADD CHECK (started_at IS NULL OR (
        started_at >= admitted_at AND
        started_at <= to_timestamp((253402300799 - 604800)::double precision) AND
        date_trunc('second', started_at) = started_at
    )),
    ADD UNIQUE (id, participant_count);

ALTER TABLE online_active_players
    DROP CONSTRAINT online_active_players_seat_check,
    ADD COLUMN participant_count SMALLINT NOT NULL DEFAULT 2,
    ADD CHECK (seat >= 0 AND seat < participant_count),
    ADD FOREIGN KEY (match_id, participant_count)
        REFERENCES online_active_matches(id, participant_count) ON DELETE CASCADE;

ALTER TABLE online_match_results
    ADD COLUMN room_id UUID CHECK (room_id <> '00000000-0000-0000-0000-000000000000'),
    ADD COLUMN participant_count SMALLINT NOT NULL DEFAULT 2 CHECK (participant_count BETWEEN 2 AND 4),
    ADD CHECK (room_id IS NOT NULL OR participant_count = 2),
    ADD UNIQUE (id, participant_count);

ALTER TABLE online_match_players
    DROP CONSTRAINT online_match_players_seat_check,
    ADD COLUMN participant_count SMALLINT NOT NULL DEFAULT 2,
    ADD COLUMN rank SMALLINT CHECK (rank BETWEEN 1 AND participant_count),
    ADD COLUMN tied BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN forfeited BOOLEAN NOT NULL DEFAULT false,
    ADD CHECK (seat >= 0 AND seat < participant_count),
    ADD CHECK (NOT tied OR rank IS NOT NULL),
    ADD CHECK (NOT forfeited OR rank IS NULL),
    ADD FOREIGN KEY (match_id, participant_count)
        REFERENCES online_match_results(id, participant_count) ON DELETE CASCADE;
