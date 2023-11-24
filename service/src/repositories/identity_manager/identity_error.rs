use crate::repositories::DBError;
use shine_service::service::PGError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum IdentityBuildError {
    #[error(transparent)]
    DBError(#[from] DBError),
}

impl From<tokio_postgres::Error> for IdentityBuildError {
    fn from(err: tokio_postgres::Error) -> Self {
        Self::DBError(err.into())
    }
}

#[derive(Debug, ThisError)]
pub enum IdentityError {
    #[error("User id already taken")]
    UserIdConflict,
    #[error("Name already taken")]
    NameConflict,
    #[error("Email already linked to a user")]
    LinkEmailConflict,
    #[error("External id already linked to a user")]
    LinkProviderConflict,
    #[error("Failed to generate token")]
    TokenConflict,
    #[error("Fingerprint is missing for the requested token kind")]
    MissingFingerprint,
    #[error("Operation failed with conflict, no change was made")]
    UpdateConflict,
    #[error("User was removed during the operation")]
    UserDeleted,
    #[error(transparent)]
    DBError(#[from] DBError),
}

impl From<PGError> for IdentityError {
    fn from(err: PGError) -> Self {
        Self::DBError(err.into())
    }
}
