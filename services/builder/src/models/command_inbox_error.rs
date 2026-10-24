use shine_infra::db::redis::RedisError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CommandInboxError {
    #[error("{0}")]
    InvalidConfig(&'static str),
    /// Redis answered with something the inbox cannot make sense of.
    #[error("Unexpected answer from Redis: {0}")]
    Malformed(String),
    #[error(transparent)]
    Redis(#[from] RedisError),
}
