CREATE TABLE identities (
    user_id UUID NOT NULL PRIMARY KEY,
    kind SMALLINT NOT NULL,
    created TIMESTAMPTZ NULL,
    name VARCHAR(64) NOT NULL,
    encrypted_email VARCHAR(512),
    email_hash VARCHAR(64),
    email_confirmed BOOLEAN NOT NULL DEFAULT False,
    profile_image TEXT
);

CREATE UNIQUE INDEX idx_name ON identities (name);

CREATE UNIQUE INDEX idx_email_hash ON identities (email_hash);
