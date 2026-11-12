// Not called by anything outside this module's own tests yet: the stream worker that drives the
// service is a later round.
#![allow(dead_code, unused_imports)]

mod stream_config;
mod stream_egress;
mod stream_kinds;
mod stream_log_service;
mod stream_metrics;
mod stream_persistence;
mod stream_registry;
mod stream_relay;
mod stream_worker;

pub use self::{
    stream_config::{StreamConfig, StreamsConfig},
    stream_egress::StreamEgress,
    stream_kinds::standard_registry,
    stream_log_service::{AppendOutcome, SnapshotOutcome, StoredStream, StreamLogService},
    stream_metrics::{stream_metrics, StreamMetrics},
    stream_persistence::{PersistResult, StreamPersistence},
    stream_registry::{StreamRegistry, StreamRegistryBuilder, MAX_COMMAND_BYTES},
    stream_relay::StreamRelay,
    stream_worker::StreamWorker,
};
