use crate::models::HubError;
use shine_infra::db::redis::RedisDBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RedisHubRegistryBuildError {
    #[error(transparent)]
    DBError(#[from] RedisDBError),
}

impl From<RedisDBError> for HubError {
    fn from(err: RedisDBError) -> Self {
        match err {
            RedisDBError::PoolError(_) | RedisDBError::RedisError(_) => {
                HubError::registry_db(err)
            }
            RedisDBError::ListenerClosed | RedisDBError::AlreadyListening(_) => {
                HubError::internal(err)
            }
        }
    }
}
