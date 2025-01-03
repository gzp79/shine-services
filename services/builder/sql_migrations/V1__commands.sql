CREATE TABLE commands(
    content_id UUID NOT NULL PRIMARY KEY,
    version INT NOT NULL,
    user_id UUID NOT NULL,
    command TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_command ON commands(content_id, version);