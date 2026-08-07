use axum::http::StatusCode;
use shine_infra::web::responses::Problem;
use std::{backtrace::Backtrace as StdBacktrace, error::Error as StdError, panic::Location};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum HubError {
    #[error("User already connected")]
    UserAlreadyConnected,
    #[error("Failed to send command to hub")]
    SendCommandFailed,
    #[error("Registry DB error")]
    RegistryDbError {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
    },
    #[error("Internal error")]
    Internal {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        // Keep this as String: using std::backtrace::Backtrace directly here triggers
        // unstable error provider plumbing in thiserror on our current toolchain.
        backtrace: StdBacktrace,
    },
}

impl HubError {
    #[track_caller]
    pub fn registry_db(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::RegistryDbError {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }

    #[track_caller]
    pub fn internal(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Internal {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }
}

impl From<HubError> for Problem {
    fn from(value: HubError) -> Self {
        match value {
            HubError::UserAlreadyConnected => Problem::new(StatusCode::CONFLICT, "User already connected"),
            HubError::SendCommandFailed => {
                Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command to hub")
            }
            err @ HubError::RegistryDbError { .. } => Problem::new(StatusCode::SERVICE_UNAVAILABLE, "hub-unavailable")
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
            err @ HubError::Internal { .. } => Problem::internal_error_ty("hub-error")
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
