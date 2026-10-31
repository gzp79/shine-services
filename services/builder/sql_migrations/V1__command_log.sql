CREATE TYPE cl_command_input AS (
    version BIGINT,
    command_id UUID,
    inbox_id TEXT,
    user_id UUID,
    data TEXT,
    command_hash BIGINT,
    chain_hash BIGINT
);

CREATE TABLE cl_streams (
    kind VARCHAR(64) NOT NULL,
    id VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    chain_hash BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, id)
);

CREATE TABLE cl_commands (
    kind VARCHAR(64) NOT NULL,
    id VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    command_id UUID NOT NULL,
    inbox_id TEXT,
    user_id UUID NOT NULL,
    data TEXT NOT NULL,
    command_hash BIGINT NOT NULL,
    chain_hash BIGINT NOT NULL,
    PRIMARY KEY (kind, id, version),
    FOREIGN KEY (kind, id) REFERENCES cl_streams (kind, id) ON DELETE CASCADE
);

CREATE OR REPLACE FUNCTION cl_commands_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'cl_commands rows are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER cl_commands_no_update
BEFORE UPDATE ON cl_commands
FOR EACH ROW
EXECUTE FUNCTION cl_commands_immutable();

CREATE TABLE cl_snapshots (
    kind VARCHAR(64) NOT NULL,
    id VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL,
    hash BIGINT NOT NULL,
    chain_hash BIGINT NOT NULL,
    data BYTEA NOT NULL,
    PRIMARY KEY (kind, id, version),
    FOREIGN KEY (kind, id, version) REFERENCES cl_commands (kind, id, version) ON DELETE CASCADE
);
