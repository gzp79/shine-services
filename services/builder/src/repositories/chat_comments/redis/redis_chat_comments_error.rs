use shine_infra::db::redis::RedisError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisChatCommentsBuildError {
    #[error(transparent)]
    DBError(#[from] RedisError),
    #[error("{0}")]
    InvalidConfig(&'static str),
}
