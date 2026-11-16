use crate::web::responses::Problem;
use std::error::Error as StdError;
use thiserror::Error as ThisError;
use tokio_postgres::error::SqlState;

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

impl PgError {
    /// Returns if PostgreSQL was out of reach and the error is likely transient.
    pub fn is_unavailable(&self) -> bool {
        match self {
            Self::CreatePoolError(_) | Self::PoolError(_) | Self::ListenerConnectTimeout => true,
            Self::PgRawError(err) => {
                err.is_closed()
                    || err.source().is_some_and(|source| source.is::<std::io::Error>())
                    || err.code().is_some_and(|code| {
                        // Class 08 is "connection exception".
                        code.code().starts_with("08")
                            || *code == SqlState::ADMIN_SHUTDOWN
                            || *code == SqlState::CRASH_SHUTDOWN
                            || *code == SqlState::CANNOT_CONNECT_NOW
                            || *code == SqlState::TOO_MANY_CONNECTIONS
                    })
            }
            Self::SqlMigration(_) | Self::ListenerClosed | Self::AlreadyListening(_) => false,
        }
    }
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
