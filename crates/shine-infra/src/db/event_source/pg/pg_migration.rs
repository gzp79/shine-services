pub fn migration_001(aggregate: &str) -> String {
    format!(
        r#"
-------------------------------------------------------------
-- Event stream
CREATE TABLE es_heads_{aggregate} (
    aggregate_id VARCHAR(256) NOT NULL PRIMARY KEY,
    version INT NOT NULL
);

-- Notify about stream changes (create, update, delete)
CREATE OR REPLACE FUNCTION notify_es_heads_{aggregate}()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'operation', 'create',
                'aggregate_id', NEW.aggregate_id
            )::text
        );
        RETURN NEW;
    ELSIF (TG_OP = 'UPDATE') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'operation', 'update',
                'aggregate_id', NEW.aggregate_id,
                'version', NEW.version
            )::text
        );
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'operation', 'delete',
                'aggregate_id', OLD.aggregate_id
            )::text
        );
        RETURN OLD;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER es_heads_{aggregate}_trigger
AFTER INSERT OR UPDATE OR DELETE ON es_heads_{aggregate}
FOR EACH ROW
EXECUTE FUNCTION notify_es_heads_{aggregate}();

-------------------------------------------------------------
-- Event stream events
CREATE TABLE es_events_{aggregate} (
    aggregate_id VARCHAR(256) NOT NULL,
    version INT NOT NULL,
    event_type VARCHAR(255) NOT NULL,
    data JSONB NOT NULL,
    PRIMARY KEY (aggregate_id, version),
    FOREIGN KEY (aggregate_id) REFERENCES es_heads_{aggregate} (aggregate_id) ON DELETE CASCADE
);

-- Prevent updates on the events, make rows immutable
CREATE OR REPLACE FUNCTION prevent_es_events_{aggregate}_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Cannot update rows in es_events_{aggregate}';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER prevent_update_es_events_{aggregate}
BEFORE UPDATE ON es_events_{aggregate}
FOR EACH ROW
EXECUTE FUNCTION prevent_es_events_{aggregate}_update();

-------------------------------------------------------------
-- Event stream snapshots
CREATE TABLE es_snapshots_{aggregate} (
    aggregate_id VARCHAR(256) NOT NULL,
    snapshot VARCHAR(255) NOT NULL,
    version INT NOT NULL,
    data JSONB NOT NULL,
    PRIMARY KEY (
        aggregate_id,
        snapshot,
        version
    ),
    FOREIGN KEY (aggregate_id) REFERENCES es_heads_{aggregate} (aggregate_id) ON DELETE CASCADE
);
"#
    )
}
