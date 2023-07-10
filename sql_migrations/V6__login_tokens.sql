CREATE TABLE login_tokens (
    user_id UUID NOT NULL,
    created TIMESTAMPTZ NULL,
    expire TIMESTAMPTZ NULL,
    token TEXT NOT NULL,
    CONSTRAINT fkey_user_id FOREIGN KEY(user_id) REFERENCES identities(user_id) ON DELETE CASCADE
);

CREATE INDEX idx_user_id ON login_tokens(user_id);
CREATE UNIQUE INDEX idx_token ON login_tokens(token);