use sqlx_interpolation::DBBuilderError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DBError {
    #[error("Operation retry count reached")]
    RetryLimitReached,
    
    #[error("Database command: {0}")]
    DBCommand(#[from] DBBuilderError),
    #[error("Database migration error")]
    SqlxMigration(#[from] sqlx::migrate::MigrateError),
    #[error("Database error")]
    SqlxError(#[from] sqlx::Error),
}
