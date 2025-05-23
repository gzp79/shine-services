CREATE TABLE external_logins (
    user_id UUID NOT NULL,
    provider TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    linked TIMESTAMPTZ NULL,
    name VARCHAR(64),
    email VARCHAR(256),
    CONSTRAINT fkey_user_id FOREIGN KEY (user_id) REFERENCES identities (user_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_provider_provider_id ON external_logins (provider, provider_id);