CREATE TABLE session_consents (
    session_id uuid PRIMARY KEY REFERENCES auth_sessions(id) ON DELETE CASCADE,
    revision uuid NOT NULL CHECK (revision <> '00000000-0000-0000-0000-000000000000'),
    checked_at bigint NOT NULL CHECK (checked_at BETWEEN 0 AND 253402300798),
    expires_at bigint NOT NULL CHECK (expires_at BETWEEN checked_at + 1 AND 253402300799),
    analytics boolean NOT NULL,
    advertising boolean NOT NULL
);
