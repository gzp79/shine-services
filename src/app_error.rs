use axum::{http::StatusCode, response::IntoResponse};
use sqlx_interpolation::DBBuilderError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AppError {
    #[error("Failed to request access token for provider {0}")]
    OAuthAccessToken(String),

    #[error("Database command: {0}")]
    DBCommand(#[from] DBBuilderError),
    #[error("Some retry operation reached the limit")]
    DBRetryLimitReached,
    #[error("Database migration error")]
    SqlxMigration(#[from] sqlx::migrate::MigrateError),
    #[error("Database error")]
    SqlxError(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::OAuthAccessToken(_) => (StatusCode::BAD_REQUEST, format!("{self:?}")).into_response(),
            AppError::DBRetryLimitReached => (StatusCode::INTERNAL_SERVER_ERROR, format!("{self:?}")).into_response(),
            AppError::DBCommand(_) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{self:?}")).into_response(),
            AppError::SqlxMigration(_) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{self:?}")).into_response(),
            AppError::SqlxError(_) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{self:?}")).into_response(),
        }
    }
}
