use base64::DecodeError;
use shine_infra::{
    crypto::{DataProtectionError, IdEncoderError},
    db::PGDBError,
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum PgIdentityBuildError {
    #[error(transparent)]
    IdEncoder(#[from] IdEncoderError),
    #[error(transparent)]
    DBError(#[from] PGDBError),
    #[error(transparent)]
    DataProtectionError(#[from] DataProtectionError),
    #[error(transparent)]
    DecodeError(#[from] DecodeError),
}
