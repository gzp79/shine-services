use opentelemetry::{
    global,
    metrics::{Counter, Histogram, UpDownCounter},
    KeyValue,
};
use shine_game::replica::StreamKind;
use shine_infra::telemetry::{label, SECONDS_BUCKETS};
use std::{sync::LazyLock, time::Instant};

/// Bucket boundaries of counts of entries (inbox depth, batch size).
const COUNT_BUCKETS: &[f64] = &[
    1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
];

/// Metrics of the command streams: the write path with its throttling, the ingest of the inbox, the
/// persistence and the lifecycle of the workers. Every instrument is labelled with the stream `kind`;
/// ids of streams, users and commands are only available in the traces.
pub struct StreamMetrics {
    /// Commands offered to the write path, by outcome (`queued`, `invalid`, `not_subscribed`, `busy`, `failed`).
    pub commands_pushed: Counter<u64>,
    /// Time a push waited for the inbox to drain, by outcome (`ok`, `timeout`). Only pushes that had to wait.
    pub push_throttle_wait: Histogram<f64>,
    /// Length of the inbox, sampled by the backpressure check.
    pub inbox_depth: Histogram<u64>,

    /// Commands taken from the inbox and versioned.
    pub commands_ingested: Counter<u64>,
    /// Inbox entries that were not taken, by reason (`malformed`, `already_stored`).
    pub commands_dropped: Counter<u64>,
    /// Duration of one pass over the inbox, polling included.
    pub ingest_duration: Histogram<f64>,
    /// Number of inbox entries a pass took.
    pub ingest_batch_size: Histogram<u64>,
    /// Age of the oldest entry of a pass when it was taken: the delay between the push and the ingest.
    pub inbox_age: Histogram<f64>,

    /// Duration of a persistence pass, by result (`advanced`, `current`, `reloaded`, `failed`).
    pub persist_duration: Histogram<f64>,
    /// Commands made durable.
    pub commands_persisted: Counter<u64>,
    /// Age of the oldest command of a persisted batch when it became durable: the delay between the push and the log.
    pub persist_delay: Histogram<f64>,
    /// Reloads of the stream history from the log, after a divergence from it.
    pub reloads: Counter<u64>,
    /// Snapshots, by outcome (`stored`, `diverged`, `skipped`).
    pub snapshots: Counter<u64>,

    /// Running stream workers.
    pub workers: UpDownCounter<i64>,
    /// Workers that ended, by reason (`stopped`, `idle`, `gave_up`).
    pub worker_stops: Counter<u64>,
    /// Connections subscribed to a stream worker.
    pub subscribers: UpDownCounter<i64>,
}

static METRICS: LazyLock<StreamMetrics> = LazyLock::new(StreamMetrics::new);

/// The metrics of the command streams. Without a configured meter provider the instruments are no-ops.
pub fn stream_metrics() -> &'static StreamMetrics {
    &METRICS
}

/// The `kind` label of the metrics.
pub fn kind_label(kind: StreamKind) -> KeyValue {
    label("kind", kind.as_str())
}

/// Seconds since `start`.
pub fn elapsed_seconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64()
}

impl StreamMetrics {
    fn new() -> Self {
        let meter = global::meter("shine-builder-stream");
        Self {
            commands_pushed: meter
                .u64_counter("stream_commands_pushed")
                .with_description("Commands offered to the stream write path")
                .build(),
            push_throttle_wait: meter
                .f64_histogram("stream_push_throttle_wait")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Time a command push waited for the stream inbox to drain")
                .build(),
            inbox_depth: meter
                .u64_histogram("stream_inbox_depth")
                .with_boundaries(COUNT_BUCKETS.to_vec())
                .with_description("Length of the stream inbox, as sampled by the backpressure check")
                .build(),

            commands_ingested: meter
                .u64_counter("stream_commands_ingested")
                .with_description("Commands taken from the stream inbox")
                .build(),
            commands_dropped: meter
                .u64_counter("stream_commands_dropped")
                .with_description("Stream inbox entries that were not taken")
                .build(),
            ingest_duration: meter
                .f64_histogram("stream_ingest_duration")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Duration of one pass over the stream inbox")
                .build(),
            ingest_batch_size: meter
                .u64_histogram("stream_ingest_batch_size")
                .with_boundaries(COUNT_BUCKETS.to_vec())
                .with_description("Number of inbox entries a pass over the stream inbox took")
                .build(),
            inbox_age: meter
                .f64_histogram("stream_inbox_age")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Age of the oldest inbox entry of a pass when it was taken")
                .build(),

            persist_duration: meter
                .f64_histogram("stream_persist_duration")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Duration of a stream persistence pass")
                .build(),
            commands_persisted: meter
                .u64_counter("stream_commands_persisted")
                .with_description("Stream commands made durable")
                .build(),
            persist_delay: meter
                .f64_histogram("stream_persist_delay")
                .with_unit("s")
                .with_boundaries(SECONDS_BUCKETS.to_vec())
                .with_description("Age of the oldest command of a persisted batch when it became durable")
                .build(),
            reloads: meter
                .u64_counter("stream_reloads")
                .with_description("Reloads of a stream history from the log")
                .build(),
            snapshots: meter
                .u64_counter("stream_snapshots")
                .with_description("Stream snapshots, by outcome")
                .build(),

            workers: meter
                .i64_up_down_counter("stream_workers")
                .with_description("Running stream workers")
                .build(),
            worker_stops: meter
                .u64_counter("stream_worker_stops")
                .with_description("Stream workers that ended")
                .build(),
            subscribers: meter
                .i64_up_down_counter("stream_subscribers")
                .with_description("Connections subscribed to a stream worker")
                .build(),
        }
    }
}
