use crate::{models::messages::SubscribeFailure, repositories::command_log::StreamHead};
use shine_game::replica::StreamKind;
use std::{backtrace::Backtrace as StdBacktrace, error::Error as StdError, panic::Location};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CommandStreamError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Too much contention appending to the stream")]
    Contention,
    #[error("Drifted from the persisted chain (head at version {})", .head.version)]
    LogDiverged { head: StreamHead },
    #[error("Snapshot drifted from the persisted chain")]
    SnapshotDiverged,
    #[error("Stored stream data is unusable: {0}")]
    Corrupt(String),
    #[error("The stream kind {0:?} is registered twice")]
    DuplicateKind(StreamKind),
    #[error("Service {service} unavailable")]
    ServiceUnavailable {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        service: &'static str,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
    },
    #[error("Internal error")]
    Internal {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
    },
}

impl CommandStreamError {
    #[track_caller]
    pub fn service_unavailable(service: &'static str, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::ServiceUnavailable {
            service,
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }

    #[track_caller]
    pub fn internal(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Internal {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }

    /// What a client is told when this is why it cannot subscribe.
    pub fn subscribe_failure(&self) -> SubscribeFailure {
        SubscribeFailure::Failed
    }
}
