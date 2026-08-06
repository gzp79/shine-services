use shine_infra::db::RedisDBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum HubConnectionError {
    #[error(transparent)]
    DBError(#[from] RedisDBError),
}
