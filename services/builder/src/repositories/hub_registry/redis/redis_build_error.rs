use shine_infra::db::DBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisHubRegistryBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}
