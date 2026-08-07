use crate::{
    db::redis::redis_connection::{RedisConnectionError, RedisRawError},
    web::responses::Problem,
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisError {
    #[error("Failed to get pooled redis connection")]
    PoolError(#[source] RedisConnectionError),
    #[error(transparent)]
    RawError(#[from] RedisRawError),
    #[error("The listener has been closed")]
    ListenerClosed,
    #[error("A handler is already registered for channel {0:?}")]
    AlreadyListening(String),
}

/// Legacy conversion kept for backward compatibility.
///
/// Prefer mapping infra errors to service-level boxed internal errors and
/// converting to `Problem` only at API boundaries.
impl From<RedisError> for Problem {
    fn from(err: RedisError) -> Self {
        match err {
            RedisError::PoolError(_) | RedisError::RawError(_) => Problem::service_unavailable()
                .with_detail("redis-unavailable")
                .with_sensitive_dbg(err),
            err => Problem::internal_error()
                .with_detail("redis-internal-error")
                .with_sensitive_dbg(err),
        }
    }
}
