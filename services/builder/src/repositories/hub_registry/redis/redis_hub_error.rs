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
                HubError::StoreUnavailable { source: Box::new(err) }
            }
            RedisDBError::ListenerClosed | RedisDBError::AlreadyListening(_) => {
                HubError::Internal { source: Box::new(err) }
            }
        }
    }
}
