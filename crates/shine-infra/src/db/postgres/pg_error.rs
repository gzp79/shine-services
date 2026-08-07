use crate::web::responses::Problem;
use thiserror::Error as ThisError;

use super::pg_connection::{PgConnectionError, PgPoolError, PgRawError};

#[derive(Debug, ThisError)]
pub enum PgError {
    #[error("Failed to get a PG connection from the pool")]
    CreatePoolError(#[source] PgConnectionError),
    #[error("Failed to get a PG connection from the pool")]
    PoolError(#[source] PgPoolError),
    #[error(transparent)]
    PgRawError(#[from] PgRawError),
    #[error(transparent)]
    SqlMigration(#[from] refinery::Error),
    #[error("The listener has been closed")]
    ListenerClosed,
    #[error("The listener connection timed out")]
    ListenerConnectTimeout,
    #[error("A handler is already registered for channel {0:?}")]
    AlreadyListening(String),
}

impl From<PgError> for Problem {
    fn from(err: PgError) -> Self {
        match err {
            PgError::CreatePoolError(_) | PgError::PoolError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
