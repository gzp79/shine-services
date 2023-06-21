use crate::db::{DBError, DBSessionError, NameGeneratorError};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum OIDCError {
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
    #[error(transparent)]
    NameGeneratorError(#[from] NameGeneratorError),
}

impl IntoResponse for OIDCError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            OIDCError::MissingSession => StatusCode::BAD_REQUEST,
            OIDCError::InconsistentSession => StatusCode::BAD_REQUEST,
            OIDCError::InvalidCsrfState => StatusCode::BAD_REQUEST,
            OIDCError::FailedTokenExchange(_) => StatusCode::BAD_REQUEST,
            OIDCError::MissingIdToken => StatusCode::BAD_REQUEST,
            OIDCError::FailedIdVerification(_) => StatusCode::BAD_REQUEST,
            OIDCError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OIDCError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OIDCError::DBSessionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OIDCError::NameGeneratorError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}
