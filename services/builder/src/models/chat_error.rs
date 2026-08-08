use std::{backtrace::Backtrace as StdBacktrace, error::Error as StdError, panic::Location};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum ChatError {
    #[error("Internal chat error")]
    Internal {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
    },
}

impl ChatError {
    #[track_caller]
    pub fn internal(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Internal {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }
}
