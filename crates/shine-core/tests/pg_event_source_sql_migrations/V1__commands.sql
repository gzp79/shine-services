CREATE TABLE es_heads_test(
    aggregate_id UUID NOT NULL PRIMARY KEY,
    version INT NOT NULL
);

CREATE OR REPLACE FUNCTION notify_es_heads_test_update()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('es_notification_test', json_build_object('aggregate_id', NEW.aggregate_id, 'version', NEW.version)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER es_heads_test_update_trigger
AFTER UPDATE ON es_heads_test
FOR EACH ROW
EXECUTE FUNCTION notify_es_heads_test_update();

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

