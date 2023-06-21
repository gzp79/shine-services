use crate::db::{DBError, DBSessionError};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AuthServiceError {
    #[error("Session cookie was missing or corrupted")]
    MissingSession,
    #[error("Cross Server did not return an ID token")]
    InvalidCsrfState,
    #[error("Session and external login cookies are not matching")]
    InconsistentSession,
    #[error("Failed to exchange authorization code to access token: {0}")]
    FailedTokenExchange(String),
    #[error("Cross-Site Request Forgery (Csrf) check failed")]
    MissingIdToken,
    #[error("Failed to verify id token: {0}")]
    FailedIdVerification(String),

    #[error("Failed to create session")]
    DBSessionError(#[from] DBSessionError),
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
}

impl IntoResponse for AuthServiceError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AuthServiceError::MissingSession => StatusCode::BAD_REQUEST,
            AuthServiceError::InconsistentSession => StatusCode::BAD_REQUEST,
            AuthServiceError::InvalidCsrfState => StatusCode::BAD_REQUEST,
            AuthServiceError::FailedTokenExchange(_) => StatusCode::BAD_REQUEST,
            AuthServiceError::MissingIdToken => StatusCode::BAD_REQUEST,
            AuthServiceError::FailedIdVerification(_) => StatusCode::BAD_REQUEST,
            AuthServiceError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthServiceError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthServiceError::DBSessionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}
