CREATE TABLE identities(
    user_id UUID NOT NULL PRIMARY KEY,
    kind INTEGER NOT NULL,
    created TIMESTAMPTZ NULL,
    name VARCHAR(64) NOT NULL,
    email VARCHAR(256),
    profile_image TEXT
);

CREATE UNIQUE INDEX idx_name ON identities(name);