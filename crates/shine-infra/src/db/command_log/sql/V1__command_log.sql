CREATE TABLE cl_streams (
    stream_id VARCHAR(64) NOT NULL PRIMARY KEY,
    kind VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    generation BIGINT NOT NULL DEFAULT 0 CHECK (generation >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- `data` is TEXT: the log never queries inside a command and must keep the canonical text byte for byte.
-- jsonb would reformat the text and reject the \u0000 escape.
CREATE TABLE cl_commands (
    stream_id VARCHAR(64) NOT NULL REFERENCES cl_streams (stream_id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    command_id UUID NOT NULL,
    user_id UUID NOT NULL,
    generation BIGINT NOT NULL,
    data TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
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
    data BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (stream_id, version),
    FOREIGN KEY (stream_id, version) REFERENCES cl_commands (stream_id, version) ON DELETE CASCADE
);
