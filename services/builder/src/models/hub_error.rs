use axum::http::StatusCode;
use shine_infra::web::responses::Problem;
use std::error::Error as StdError;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum HubError {
    #[error("User already connected")]
    UserAlreadyConnected,
    #[error("Failed to send command to hub")]
    SendCommandFailed,
    #[error("Hub registry store unavailable")]
    StoreUnavailable {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("Internal error")]
    Internal {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}

impl From<HubError> for Problem {
    fn from(value: HubError) -> Self {
        match value {
            HubError::UserAlreadyConnected => Problem::new(StatusCode::CONFLICT, "User already connected"),
            HubError::SendCommandFailed => {
                Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command to hub")
            }
            err @ HubError::StoreUnavailable { .. } => Problem::service_unavailable()
                .with_detail("hub-unavailable")
                .with_sensitive_dbg(err),
            err @ HubError::Internal { .. } => Problem::internal_error_ty("hub-error")
                .with_detail("hub-internal-error")
                .with_sensitive_dbg(err),
        }
    }
}
