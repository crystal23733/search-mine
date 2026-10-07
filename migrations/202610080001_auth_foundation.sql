CREATE TABLE auth_accounts (
    id uuid PRIMARY KEY,
    nickname text CHECK (nickname IS NULL OR octet_length(nickname) BETWEEN 2 AND 128),
    created_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL CHECK (last_seen_at >= created_at)
);

CREATE TABLE auth_identities (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES auth_accounts(id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider IN ('google', 'apple', 'kakao', 'naver')),
    issuer text NOT NULL CHECK (
        (provider = 'google' AND issuer = 'https://accounts.google.com') OR
        (provider = 'apple' AND issuer = 'https://appleid.apple.com') OR
        (provider = 'kakao' AND issuer = 'https://kauth.kakao.com') OR
        (provider = 'naver' AND issuer = 'https://nid.naver.com')
    ),
    subject_digest bytea NOT NULL CHECK (octet_length(subject_digest) = 32),
    digest_key_version integer NOT NULL CHECK (digest_key_version > 0),
    linked_at timestamptz NOT NULL,
    UNIQUE (provider, issuer, subject_digest, digest_key_version),
    UNIQUE (id, provider)
);
CREATE INDEX auth_identities_account ON auth_identities(account_id);

CREATE TABLE auth_credentials (
    identity_id uuid PRIMARY KEY,
    provider text NOT NULL DEFAULT 'apple' CHECK (provider = 'apple'),
    encrypted_refresh_token bytea NOT NULL CHECK (octet_length(encrypted_refresh_token) BETWEEN 34 AND 4129),
    updated_at timestamptz NOT NULL,
    FOREIGN KEY (identity_id, provider) REFERENCES auth_identities(id, provider) ON DELETE CASCADE
);

CREATE TABLE auth_sessions (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES auth_accounts(id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    created_at timestamptz NOT NULL,
    authenticated_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    CHECK (expires_at > created_at AND expires_at <= created_at + interval '30 days'),
    CHECK (authenticated_at >= created_at AND authenticated_at < expires_at)
);
CREATE INDEX auth_sessions_account ON auth_sessions(account_id);
CREATE INDEX auth_sessions_expiry ON auth_sessions(expires_at);

CREATE TABLE auth_transactions (
    id uuid PRIMARY KEY,
    state_hash bytea NOT NULL UNIQUE CHECK (octet_length(state_hash) = 32),
    browser_hash bytea NOT NULL CHECK (octet_length(browser_hash) = 32),
    nonce_hash bytea NOT NULL CHECK (octet_length(nonce_hash) = 32),
    provider text NOT NULL CHECK (provider IN ('google', 'apple', 'kakao', 'naver')),
    intent text NOT NULL CHECK (intent IN ('login', 'link', 'reauth')),
    account_id uuid REFERENCES auth_accounts(id) ON DELETE CASCADE,
    return_path text NOT NULL CHECK (return_path IN ('home', 'daily', 'friends', 'settings')),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    encrypted_verifier bytea CHECK (encrypted_verifier IS NULL OR octet_length(encrypted_verifier) BETWEEN 34 AND 4129),
    CHECK ((intent = 'login' AND account_id IS NULL) OR (intent IN ('link', 'reauth') AND account_id IS NOT NULL)),
    CHECK (expires_at > created_at AND expires_at <= created_at + interval '5 minutes'),
    CHECK ((provider IN ('google', 'kakao') AND encrypted_verifier IS NOT NULL) OR (provider IN ('apple', 'naver') AND encrypted_verifier IS NULL))
);
CREATE INDEX auth_transactions_expiry ON auth_transactions(expires_at);
