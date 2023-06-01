use crate::db::DBConnectionError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DBError {
    #[error("Operation retry count reached")]
    RetryLimitReached,
    #[error("DB has some inconsistency: {0}")]
    Inconsistency(String),
    #[error("Some constraint is violated: {0}")]
    Conflict(String),

    #[error(transparent)]
    PoolError(#[from] bb8_postgres::tokio_postgres::Error),
    #[error(transparent)]
    PooledConnectionError(#[from] DBConnectionError),
    #[error(transparent)]
    Migration(#[from] refinery::Error),
}
