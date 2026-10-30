-- Local accounts. `password` holds an Argon2id PHC hash, never the plain password.
CREATE TABLE member (
    id UUID PRIMARY KEY,
    username TEXT NOT NULL UNIQUE CHECK (username ~ '^[a-z0-9_.-]{3,32}$'),
    password TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

-- Only the SHA-256 of each login token is stored; deleting the row logs out.
CREATE TABLE auth_session (
    token_hash BYTEA PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    member_id UUID NOT NULL REFERENCES member(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX auth_session_member ON auth_session(member_id);
CREATE INDEX auth_session_expiry ON auth_session(expires_at);
