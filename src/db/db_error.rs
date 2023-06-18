use crate::db::{PGConnectionError, RedisConnectionError};
use thiserror::Error as ThisError;
use tokio_postgres::error::SqlState;

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

pub trait PGErrorChecks {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool;
}

impl PGErrorChecks for tokio_postgres::Error {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool {
        if let Some(err) = self.as_db_error() {
            if &SqlState::UNIQUE_VIOLATION == err.code()
                && err.table() == Some(table)
                && err.message().contains(constraint)
            {
                return true;
            }
        }
        false
    }
}
