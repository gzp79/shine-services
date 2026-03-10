use base64::DecodeError;
use shine_infra::{
    crypto::{DataProtectionError, IdEncoderError},
    db::DBError,
};
use thiserror::Error as ThisError;

// Re-export IdentityError from models for backward compatibility with repository internal usage
pub use crate::models::IdentityError;

#[derive(Debug, ThisError)]
pub enum IdentityBuildError {
    #[error(transparent)]
    IdEncoder(#[from] IdEncoderError),
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    DataProtectionError(#[from] DataProtectionError),
    #[error(transparent)]
    DecodeError(#[from] DecodeError),
}
