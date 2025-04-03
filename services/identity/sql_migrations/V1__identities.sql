CREATE TABLE identities (
    user_id UUID NOT NULL PRIMARY KEY,
    kind SMALLINT NOT NULL,
    created TIMESTAMPTZ NULL,
    name VARCHAR(64) NOT NULL,
    email VARCHAR(256),
    email_confirmed BOOLEAN NOT NULL DEFAULT False,
    profile_image TEXT
);

CREATE UNIQUE INDEX idx_name ON identities (name);

CREATE UNIQUE INDEX idx_email ON identities (email);