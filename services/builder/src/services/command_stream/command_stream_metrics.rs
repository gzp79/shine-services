use opentelemetry::{global, metrics::Counter};
use std::sync::LazyLock;

/// Metrics of the command streams. Labels have a small fixed set of values; stream ids are only in the traces.
pub struct CommandStreamMetrics {
    /// Commands received from the clients, by outcome.
    pub ingested: Counter<u64>,
    /// Snapshots that disagreed with the authoritative one: a determinism bug.
    pub drift: Counter<u64>,
    /// Stored commands that could not be decoded: the stream is unavailable.
    pub decode_errors: Counter<u64>,
    /// Snapshot reads over HTTP, by outcome.
    pub snapshot_reads: Counter<u64>,
}

static METRICS: LazyLock<CommandStreamMetrics> = LazyLock::new(CommandStreamMetrics::new);

/// The metrics of the command streams. Without a configured meter provider the instruments are no-ops.
pub fn command_stream_metrics() -> &'static CommandStreamMetrics {
    &METRICS
}

impl CommandStreamMetrics {
    fn new() -> Self {
        let meter = global::meter("shine-builder-command-stream");
        Self {
            ingested: meter
                .u64_counter("command_stream_ingested")
                .with_description("Commands received from the clients")
                .build(),
            drift: meter
                .u64_counter("command_stream_drift")
                .with_description("Snapshots that disagreed with the authoritative one")
                .build(),
            decode_errors: meter
                .u64_counter("command_stream_decode_errors")
                .with_description("Stored commands that could not be decoded")
                .build(),
            snapshot_reads: meter
                .u64_counter("command_stream_snapshot_reads")
                .with_description("Snapshot reads over HTTP")
                .build(),
        }
    }
}
