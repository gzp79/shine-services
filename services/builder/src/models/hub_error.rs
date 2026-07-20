use axum::http::StatusCode;
use shine_infra::web::responses::Problem;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum HubError {
    #[error("User already connected")]
    UserAlreadyConnected,
    #[error("Failed to send command to hub")]
    SendCommandFailed,
}

impl From<HubError> for Problem {
    fn from(value: HubError) -> Self {
        match value {
            HubError::UserAlreadyConnected => Problem::new(StatusCode::CONFLICT, "User already connected"),
            HubError::SendCommandFailed => {
                Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command to hub")
            }
        }
    }
}
