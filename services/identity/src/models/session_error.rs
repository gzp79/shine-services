use shine_infra::{db::redis::RedisError, session::SessionKeyError, web::responses::Problem};
use std::{backtrace::Backtrace as StdBacktrace, error::Error as StdError, panic::Location};
use thiserror::Error as ThisError;

mod pr {
    pub const KEY_CONFLICT: &str = "session-key-conflict";
}

#[derive(Debug, ThisError)]
pub enum SessionError {
    #[error("Failed to create session, conflicting keys")]
    KeyConflict,
    #[error("Error in the stored key")]
    InvalidKey,

    #[error(transparent)]
    SessionKeyError(#[from] SessionKeyError),
    #[error("Internal error")]
    InternalError {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
        unavailable: bool,
    },
}

impl SessionError {
    #[track_caller]
    fn internal_error(source: impl StdError + Send + Sync + 'static, unavailable: bool) -> Self {
        Self::InternalError {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
            unavailable,
        }
    }
}

impl From<RedisError> for SessionError {
    #[track_caller]
    fn from(err: RedisError) -> Self {
        match err {
            RedisError::PoolError(_) | RedisError::RawError(_) => Self::internal_error(err, true),
            _ => Self::internal_error(err, false),
        }
    }
}

impl From<SessionError> for Problem {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::KeyConflict => Problem::conflict(pr::KEY_CONFLICT).with_detail(err.to_string()),
            err @ SessionError::InternalError { unavailable: true, .. } => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err @ SessionError::InternalError { .. } => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
