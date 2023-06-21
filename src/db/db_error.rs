use shine_service::service::{PGConnectionError, RedisConnectionError};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DBError {
    #[error("Operation retry count reached")]
    RetryLimitReached,

    #[error("Failed to get a PG connection from the pool")]
    PostgresPoolError(#[source] PGConnectionError),
    #[error(transparent)]
    PostgresError(#[from] tokio_postgres::Error),
    #[error(transparent)]
    SqlMigration(#[from] refinery::Error),

    #[error("Failed to get pooled redis connection")]
    RedisPoolError(#[source] RedisConnectionError),
    #[error(transparent)]
    RedisError(#[from] redis::RedisError),
}
