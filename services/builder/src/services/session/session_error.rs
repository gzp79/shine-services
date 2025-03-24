use axum::http::StatusCode;
use shine_infra::web::Problem;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum SessionError {
    #[error("User already connected")]
    UserAlreadyConnected,
}

impl From<SessionError> for Problem {
    fn from(value: SessionError) -> Self {
        match value {
            SessionError::UserAlreadyConnected => Problem::new(StatusCode::CONFLICT, "User already connected"),
        }
    }
}
