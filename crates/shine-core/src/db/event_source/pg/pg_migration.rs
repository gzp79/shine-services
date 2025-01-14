pub fn migration_001(aggregate: &str) -> String {
    format!(
        r#"
CREATE TABLE es_heads_{aggregate} (
    aggregate_id UUID NOT NULL PRIMARY KEY,
    version INT NOT NULL
);

CREATE OR REPLACE FUNCTION notify_es_heads_{aggregate}_update()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        PERFORM pg_notify('es_notification_{aggregate}', json_build_object('operation', 'insert', 'aggregate_id', NEW.aggregate_id)::text);
        RETURN NEW;
    ELSIF (TG_OP = 'UPDATE') THEN
        PERFORM pg_notify('es_notification_{aggregate}', json_build_object('operation', 'update', 'aggregate_id', NEW.aggregate_id, 'version', NEW.version)::text);
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        PERFORM pg_notify('es_notification_{aggregate}', json_build_object('operation', 'delete', 'aggregate_id', OLD.aggregate_id)::text);
        RETURN OLD;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER es_heads_{aggregate}_trigger
AFTER INSERT OR UPDATE OR DELETE ON es_heads_{aggregate}
FOR EACH ROW
EXECUTE FUNCTION notify_es_heads_{aggregate}_update();

CREATE TABLE es_events_{aggregate} (
    aggregate_id UUID NOT NULL,
    version INT NOT NULL,
    event_type VARCHAR(255) NOT NULL,
    data JSONB NOT NULL,
    PRIMARY KEY (aggregate_id, version),
    FOREIGN KEY (aggregate_id) REFERENCES es_heads_{aggregate} (aggregate_id) ON DELETE CASCADE
);

CREATE TABLE es_snapshots_{aggregate} (
    aggregate_id UUID NOT NULL,
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

-- Create a function to prevent updates
CREATE OR REPLACE FUNCTION prevent_es_events_{aggregate}_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Cannot update rows in es_events_{aggregate}';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
"#
    )
}
