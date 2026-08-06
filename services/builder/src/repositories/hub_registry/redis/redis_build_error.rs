use shine_infra::db::RedisDBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisHubRegistryBuildError {
    #[error(transparent)]
    DBError(#[from] RedisDBError),
}
