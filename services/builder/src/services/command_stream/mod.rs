//! Replicated, replayable command streams: the service, the kinds and the stream workers.

mod command_stream_metrics;
mod command_stream_service;
mod stream_access;
mod stream_core;
mod stream_egress;
mod stream_kind;
mod stream_worker;
#[cfg(test)]
pub(crate) mod test_utils;

pub use self::{
    command_stream_metrics::command_stream_metrics,
    command_stream_service::{
        CommandStreamService, IngestOutcome, OpenError, OpenedStream, PersistOutcome, StreamStores,
    },
    stream_access::{AllowAll, StreamAccess},
    stream_core::{Captured, Fault, Observed, StreamCore, FRAME_COMMANDS},
    stream_egress::StreamEgress,
    stream_kind::{ReplayKind, StreamKind, StreamKindRegistry},
    stream_worker::{StreamWorker, WorkerCommand, WorkerExit, WorkerFuture, WorkerHandle},
};
