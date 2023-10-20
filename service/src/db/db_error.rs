use shine_service::service::{PGConnectionError, PGCreatePoolError, PGError, RedisConnectionError};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DBError {
    #[error("Failed to get a PG connection from the pool")]
    PGCreatePoolError(#[source] PGCreatePoolError),
    #[error("Failed to get a PG connection from the pool")]
    PGPoolError(#[source] PGConnectionError),
    #[error(transparent)]
    PGError(#[from] PGError),
    #[error(transparent)]
    SqlMigration(#[from] refinery::Error),

    #[error("Failed to get pooled redis connection")]
    RedisPoolError(#[source] RedisConnectionError),
    #[error(transparent)]
    RedisError(#[from] redis::RedisError),
}
