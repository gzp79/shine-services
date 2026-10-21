//! Replicated, replayable command streams: the service, the kinds and the stream workers.

mod chat;
mod command_stream_metrics;
mod command_stream_service;
mod stream_access;
mod stream_core;
mod stream_egress;
#[cfg(test)]
mod stream_integration_test;
mod stream_kind;
mod stream_worker;
#[cfg(test)]
pub(crate) mod test_utils;

pub use self::{
    chat::chat_access,
    command_stream_metrics::command_stream_metrics,
    command_stream_service::{CommandStreamService, IngestOutcome, OpenError, StreamStores},
    stream_access::{AllowList, StreamAccess},
    stream_core::{Fault, Observed, StreamCore},
    stream_egress::StreamEgress,
    stream_kind::{StreamKind, StreamKindRegistry},
    stream_worker::{StreamWorker, WorkerCommand, WorkerExit, WorkerFuture, WorkerHandle},
};

#[cfg(test)]
pub use self::stream_access::AllowAll;
