use super::{PGConnectionError, PGCreatePoolError, PGError, RedisConnectionError};
use crate::web::responses::Problem;
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

impl From<DBError> for Problem {
    fn from(err: DBError) -> Self {
        match err {
            DBError::PGCreatePoolError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            DBError::PGPoolError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            DBError::RedisPoolError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            DBError::RedisError(_) => Problem::service_unavailable()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),

            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
