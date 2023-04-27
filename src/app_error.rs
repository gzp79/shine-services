use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use oauth2::url;
use shine_service::axum::session::SessionError;
use sqlx_interpolation::DBBuilderError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AppError {
    #[error(transparent)]
    InvalidSessionMeta(#[from] SessionError),

    #[error("Invalid authorization endpoint URL")]
    AuthUrlError(url::ParseError),
    #[error("Invalid token endpoint URL")]
    TokenUrlError(url::ParseError),
    #[error("Invalid redirect URL")]
    RedirectUrlError(url::ParseError),

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
    fn into_response(self) -> Response {
        let status_code = match &self {
            AppError::InvalidSessionMeta(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::AuthUrlError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::TokenUrlError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::RedirectUrlError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::DBRetryLimitReached => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::DBCommand(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SqlxMigration(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SqlxError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}
