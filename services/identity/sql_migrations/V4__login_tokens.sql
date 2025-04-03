CREATE TABLE login_tokens (
    user_id UUID NOT NULL,
    kind SMALLINT NOT NULL,
    email VARCHAR(255) NULL,
    token TEXT NOT NULL,
    created TIMESTAMPTZ NULL,
    expire TIMESTAMPTZ NULL,
    fingerprint TEXT,
    agent TEXT NOT NULL,
    country TEXT default NULL,
    region TEXT default NULL,
    city TEXT default NULL,
    CONSTRAINT fkey_user_id FOREIGN KEY (user_id) REFERENCES identities (user_id) ON DELETE CASCADE,
    -- fingerprint is required for Access token
    CONSTRAINT chk_required_fingerprint CHECK (
        kind not in (3)
        OR NOT (
            fingerprint IS NULL
            OR fingerprint = ''
        )
    ),
    -- a bound email is required for EmailAccess token
    CONSTRAINT chk_required_email CHECK (
        kind not in (4)
        OR NOT (
            email IS NULL
            OR email = ''
        )
    )
);

CREATE INDEX idx_user_id ON login_tokens (user_id);

CREATE UNIQUE INDEX idx_token ON login_tokens (token);