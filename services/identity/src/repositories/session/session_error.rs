use shine_core::{db::DBError, web::SessionKeyError};
use thiserror::Error as ThisError;

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
