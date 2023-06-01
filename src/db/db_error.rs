use crate::db::DBConnectionError;
use thiserror::Error as ThisError;
use tokio_postgres::error::SqlState;

#[derive(Debug, ThisError)]
pub enum DBError {
    #[error("Operation retry count reached")]
    RetryLimitReached,
    #[error("DB has some inconsistency: {0}")]
    Inconsistency(String),
    #[error("Some constraint is violated: {0}")]
    Conflict(String),

    #[error(transparent)]
    PooledConnectionError(#[from] DBConnectionError),
    #[error(transparent)]
    Migration(#[from] refinery::Error),
    #[error(transparent)]
    PostgresError(#[from] tokio_postgres::Error),
}


pub trait DBErrorChecks {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool;
}

impl DBErrorChecks for tokio_postgres::Error {
    fn is_constraint(&self, table: &str, constraint: &str) -> bool {
        if let Some(err) = self.as_db_error() {
            if &SqlState::UNIQUE_VIOLATION == err.code() {
                if err.table() == Some("identities") && err.message().contains("idx_name") {
                    return true;
                }
            }
        }
        false
    }
}
