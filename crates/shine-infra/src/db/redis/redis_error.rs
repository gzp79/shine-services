use crate::web::responses::Problem;
use thiserror::Error as ThisError;

use super::redis_connection::RedisConnectionError;

#[derive(Debug, ThisError)]
pub enum RedisDBError {
    #[error("Failed to get pooled redis connection")]
    PoolError(#[source] RedisConnectionError),
    #[error(transparent)]
    RedisError(#[from] redis::RedisError),
    #[error("The listener has been closed")]
    ListenerClosed,
    #[error("A handler is already registered for channel {0:?}")]
    AlreadyListening(String),
}

impl From<RedisDBError> for Problem {
    fn from(err: RedisDBError) -> Self {
        match err {
            RedisDBError::PoolError(_) | RedisDBError::RedisError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
