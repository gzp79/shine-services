use crate::web::responses::Problem;
use thiserror::Error as ThisError;

use super::pg_connection::{PGConnectionError, PGCreatePoolError, PGError};

#[derive(Debug, ThisError)]
pub enum PGDBError {
    #[error("Failed to get a PG connection from the pool")]
    CreatePoolError(#[source] PGCreatePoolError),
    #[error("Failed to get a PG connection from the pool")]
    PoolError(#[source] PGConnectionError),
    #[error(transparent)]
    PGError(#[from] PGError),
    #[error(transparent)]
    SqlMigration(#[from] refinery::Error),
    #[error("The listener has been closed")]
    ListenerClosed,
    #[error("The listener connection timed out")]
    ListenerConnectTimeout,
    #[error("A handler is already registered for channel {0:?}")]
    AlreadyListening(String),
}

impl From<PGDBError> for Problem {
    fn from(err: PGDBError) -> Self {
        match err {
            PGDBError::CreatePoolError(_) | PGDBError::PoolError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
