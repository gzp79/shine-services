CREATE TABLE es_heads_test(
    aggregate_id UUID NOT NULL PRIMARY KEY,
    version INT NOT NULL
);

CREATE TABLE es_events_test(
    aggregate_id UUID NOT NULL PRIMARY KEY,
    version INT NOT NULL,
    type VARCHAR(255) NOT NULL,
    command JBOD NOT NULL
);

CREATE UNIQUE INDEX idx_es_events_test ON es_events_test(aggregate_id, version);