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

impl SessionError {
    #[track_caller]
    fn service_unavailable(service: &'static str, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::ServiceUnavailable {
            service,
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }

    #[track_caller]
    fn internal(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Internal {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }
}

impl From<RedisError> for SessionError {
    #[track_caller]
    fn from(err: RedisError) -> Self {
        match err {
            RedisError::PoolError(_) | RedisError::RawError(_) => Self::service_unavailable("Redis", err),
            _ => Self::internal(err),
        }
    }
}

impl From<SessionError> for Problem {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::KeyConflict => Problem::conflict(pr::KEY_CONFLICT).with_detail(err.to_string()),
            err @ SessionError::ServiceUnavailable { .. } => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err @ SessionError::Internal { .. } => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
