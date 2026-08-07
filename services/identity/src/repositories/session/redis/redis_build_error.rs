use shine_infra::db::redis::RedisError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisSessionBuildError {
    #[error(transparent)]
    DBError(#[from] RedisError),
}
