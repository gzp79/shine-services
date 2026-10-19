use shine_infra::db::{command_log::CommandLogError, redis::RedisError};
use std::{backtrace::Backtrace as StdBacktrace, error::Error as StdError, panic::Location};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum CommandStreamError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Stream is not ready in Redis")]
    NotReady,
    #[error("The user may not read the stream")]
    Forbidden,
    #[error("Internal command stream error")]
    Internal {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
    },
}

impl CommandStreamError {
    #[track_caller]
    pub fn internal(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Internal {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }
}

impl From<CommandLogError> for CommandStreamError {
    #[track_caller]
    fn from(err: CommandLogError) -> Self {
        match err {
            CommandLogError::StreamNotFound => Self::StreamNotFound,
            err => Self::internal(err),
        }
    }
}

impl From<RedisError> for CommandStreamError {
    #[track_caller]
    fn from(err: RedisError) -> Self {
        Self::internal(err)
    }
}
