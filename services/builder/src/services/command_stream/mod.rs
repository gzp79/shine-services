// Not called by anything outside this module's own tests yet: the stream worker that drives the
// service is a later round.
#![allow(dead_code, unused_imports)]

mod command_stream_config;
mod command_stream_service;
mod stream_actor;
mod stream_egress;
mod stream_kinds;
mod stream_log_service;
mod stream_persistence;
mod stream_registry;
mod stream_state;
mod stream_worker;
mod stream_worker_2;

pub use self::{
    command_stream_config::CommandStreamConfig,
    command_stream_service::{CommandStreamService, ReplayOutcome, SyncOutcome},
    stream_actor::StreamActor,
    stream_egress::StreamEgress,
    stream_kinds::standard_registry,
    stream_log_service::{AppendOutcome, SnapshotOutcome, StoredStream, StreamLogService},
    stream_persistence::{PersistResult, StreamPersistence},
    stream_registry::{StreamRegistry, StreamRegistryBuilder, MAX_COMMAND_BYTES},
    stream_state::StreamState,
    stream_worker::{spawn_worker, WorkerConfig, WorkerHandle},
    stream_worker_2::StreamWorker,
};

#[cfg(test)]
mod command_stream_service_test;
#[cfg(test)]
mod stream_log_service_test;
#[cfg(test)]
mod stream_registry_test;
#[cfg(test)]
mod stream_worker_test;
