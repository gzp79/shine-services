use axum::{http::StatusCode, response::IntoResponse};
use sqlx_interpolation::DBBuilderError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AppError {
    #[error("Database command: {0}")]
    DBCommand(#[from] DBBuilderError),
    #[error("Database migration error")]
    SqlxMigration(#[from] sqlx::migrate::MigrateError),
    #[error("Database error")]
    SqlxError(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::DBCommand(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")).into_response(),
            AppError::SqlxMigration(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")).into_response(),
            AppError::SqlxError(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")).into_response(),
        }
    }
}
