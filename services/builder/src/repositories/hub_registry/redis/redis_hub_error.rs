use crate::models::HubError;
use shine_infra::db::redis::RedisError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisHubRegistryBuildError {
    #[error(transparent)]
    DBError(#[from] RedisError),
}

impl From<RedisError> for HubError {
    fn from(err: RedisError) -> Self {
        match err {
            RedisError::PoolError(_) | RedisError::RawError(_) => HubError::registry_db(err),
            RedisError::ListenerClosed | RedisError::AlreadyListening(_) => HubError::internal(err),
        }
    }
}
