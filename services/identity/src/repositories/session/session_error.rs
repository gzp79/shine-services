use shine_infra::{
    db::DBError,
    web::{responses::Problem, session::SessionKeyError},
};
use thiserror::Error as ThisError;

mod pr {
    pub const KEY_CONFLICT: &str = "session-key-conflict";
}

#[derive(Debug, ThisError)]
pub enum SessionBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

#[derive(Debug, ThisError)]
pub enum SessionError {
    #[error("Failed to create session, conflicting keys")]
    KeyConflict,
    #[error("Error in the stored key")]
    InvalidKey,

    #[error(transparent)]
    SessionKeyError(#[from] SessionKeyError),
    #[error(transparent)]
    DBError(#[from] DBError),
}

impl From<SessionError> for Problem {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::KeyConflict => Problem::conflict(pr::KEY_CONFLICT).with_detail(err.to_string()),

            err => Problem::internal_error()
                .with_detail(err.to_string())
                .with_sensitive_dbg(err),
        }
    }
}
