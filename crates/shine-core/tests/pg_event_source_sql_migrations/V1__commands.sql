CREATE TABLE es_heads_test(
    aggregate_id UUID NOT NULL PRIMARY KEY,
    version INT NOT NULL
);

CREATE TABLE es_events_test(
    aggregate_id UUID NOT NULL,
    version INT NOT NULL,
    event_type VARCHAR(255) NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (aggregate_id, version),
    FOREIGN KEY (aggregate_id) REFERENCES es_heads_test(aggregate_id) ON DELETE CASCADE
);

CREATE TABLE es_snapshots_test(
    aggregate_id UUID NOT NULL,
    snapshot VARCHAR(255) NOT NULL,
    version INT NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (aggregate_id, snapshot, version),
    FOREIGN KEY (aggregate_id) REFERENCES es_heads_test(aggregate_id) ON DELETE CASCADE
);

-- Create a function to prevent updates
CREATE OR REPLACE FUNCTION prevent_es_events_test_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Cannot update rows in es_events_test';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
