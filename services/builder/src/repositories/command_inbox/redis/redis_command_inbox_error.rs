use crate::models::CommandStreamError;
use shine_infra::db::redis::RedisError;
use thiserror::Error as ThisError;

/// What can go wrong in the Redis command inbox. It is the repository's own error; the
/// `CommandInboxDb` implementation turns it into a `CommandStreamError` at the trait boundary, so
/// the model never sees the store it came from.
#[derive(Debug, ThisError)]
pub enum RedisCommandInboxError {
    #[error("{0}")]
    InvalidConfig(&'static str),
    /// Redis answered with something the inbox cannot make sense of.
    #[error("Unexpected answer from Redis: {0}")]
    Malformed(String),
    #[error(transparent)]
    Redis(#[from] RedisError),
}

impl From<RedisCommandInboxError> for CommandStreamError {
    #[track_caller]
    fn from(err: RedisCommandInboxError) -> Self {
        match err {
            // Unreadable store content is a stream-level fact the model acts on.
            RedisCommandInboxError::Malformed(reason) => CommandStreamError::Corrupt(reason),
            // A pool that cannot hand out a connection is a transient outage, not a bug.
            RedisCommandInboxError::Redis(err @ RedisError::PoolError(_)) => {
                CommandStreamError::service_unavailable("Redis", err)
            }
            err => CommandStreamError::internal(err),
        }
    }
}
