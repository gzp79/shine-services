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
    stream_id VARCHAR(64) NOT NULL PRIMARY KEY,
    kind VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    chain_hash BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE cl_commands (
    stream_id VARCHAR(64) NOT NULL REFERENCES cl_streams (stream_id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    command_id UUID NOT NULL,
    inbox_id TEXT,
    user_id UUID NOT NULL,
    data TEXT NOT NULL,
    command_hash BIGINT NOT NULL,
    chain_hash BIGINT NOT NULL,
    PRIMARY KEY (stream_id, version)
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
    stream_id VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL,
    hash BIGINT NOT NULL,
    chain_hash BIGINT NOT NULL,
    data BYTEA NOT NULL,
    PRIMARY KEY (stream_id, version),
    FOREIGN KEY (stream_id, version) REFERENCES cl_commands (stream_id, version) ON DELETE CASCADE
);
