use crate::db::DBError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shine_service::axum::session::SessionError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum AppError {
    #[error(transparent)]
    InvalidSessionMeta(#[from] SessionError),    

    #[error(transparent)]
    DBError(#[from] DBError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AppError::InvalidSessionMeta(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}
