pub fn migration_001(aggregate: &str) -> String {
    format!(
        r#"
-------------------------------------------------------------
-- Event stream
CREATE TABLE es_heads_{aggregate} (
    aggregate_id VARCHAR(256) NOT NULL PRIMARY KEY,
    version INT NOT NULL CHECK (version >= 0)
);

-- Notify about stream version changes (create, update, delete)
CREATE OR REPLACE FUNCTION notify_es_heads_{aggregate}()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'type', 'stream',
                'operation', 'create',
                'aggregate_id', NEW.aggregate_id,
                'version', NEW.version
            )::text );
        RETURN NEW;
    ELSIF (TG_OP = 'UPDATE') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'type', 'stream',
                'operation', 'update',
                'aggregate_id', NEW.aggregate_id,
                'version', NEW.version
            )::text );
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'type', 'stream',
                'operation', 'delete',
                'aggregate_id', OLD.aggregate_id
            )::text );
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
    version INT NOT NULL CHECK (version >= 0),
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
    start_version INT NOT NULL CHECK (start_version >= 0),
    version INT NOT NULL CHECK (version > start_version),
    data JSONB NOT NULL,

    PRIMARY KEY (aggregate_id, snapshot, version),
    FOREIGN KEY (aggregate_id) REFERENCES es_heads_{aggregate} (aggregate_id) ON DELETE CASCADE,
    FOREIGN KEY (aggregate_id, version) REFERENCES es_events_{aggregate} (aggregate_id, version) ON DELETE CASCADE,
    CONSTRAINT es_snapshots_{aggregate}_no_branching UNIQUE (aggregate_id, snapshot, start_version)
);

-- Trigger function to enforce root constraint: 
--   all but the minimal start_version must reference another snapshot
--   the root (minimal start_version) must not reference another snapshot
--   start_version can be any non-negative integer
CREATE OR REPLACE FUNCTION check_es_snapshots_{aggregate}_root()
RETURNS TRIGGER AS $$
DECLARE 
    min_start_version INT;
BEGIN
    -- Find root 
    SELECT COALESCE(MIN(start_version), NEW.start_version)
    INTO min_start_version
    FROM es_snapshots_{aggregate}
    WHERE aggregate_id = NEW.aggregate_id and snapshot = NEW.snapshot;

    -- Make sure chain is not broken for non-root snapshots 
    IF NEW.start_version != min_start_version THEN
        IF NOT EXISTS (
            SELECT 1
            FROM es_snapshots_{aggregate}
            WHERE aggregate_id = NEW.aggregate_id
              AND snapshot = NEW.snapshot
              AND version = NEW.start_version
        ) THEN
            RAISE EXCEPTION 'Snapshot chain is broken. min: %, new: %', min_start_version, NEW.start_version;
        END IF;
    END IF;
   
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to enforce the constraint
CREATE TRIGGER enforce_es_snapshots_{aggregate}_root
BEFORE INSERT OR UPDATE ON es_snapshots_{aggregate}
FOR EACH ROW
EXECUTE FUNCTION check_es_snapshots_{aggregate}_root();

-- Notify about snapshot changes (create, delete)
CREATE OR REPLACE FUNCTION notify_es_snapshots_{aggregate}()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'type', 'snapshot',
                'operation', 'create',
                'aggregate_id', NEW.aggregate_id,
                'snapshot', NEW.snapshot,
                'version', NEW.version
            )::text );
        RETURN NEW;    
    ELSIF (TG_OP = 'DELETE') THEN
        PERFORM pg_notify(
            'es_notification_{aggregate}',
            json_build_object(
                'type', 'snapshot',
                'operation', 'delete',
                'aggregate_id', OLD.aggregate_id,
                'snapshot', OLD.snapshot,
                'version', OLD.version
            )::text );
        RETURN OLD;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER es_snapshots_{aggregate}_trigger
AFTER INSERT OR DELETE ON es_snapshots_{aggregate}
FOR EACH ROW
EXECUTE FUNCTION notify_es_snapshots_{aggregate}();
"#
    )
}
