use shine_infra::db::redis::RedisError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CommandInboxError {
    #[error("{0}")]
    InvalidConfig(&'static str),
    #[error(transparent)]
    Redis(#[from] RedisError),
}
