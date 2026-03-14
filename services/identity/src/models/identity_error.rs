use shine_infra::{
    crypto::{DataProtectionError, IdEncoderError},
    db::DBError,
    web::responses::Problem,
};
use thiserror::Error as ThisError;

mod pr {
    pub const ID_CONFLICT: &str = "identity-id-conflict";
    pub const NAME_CONFLICT: &str = "identity-name-conflict";
    pub const NAME_TOO_LONG: &str = "identity-name-too-long";
    pub const EMAIL_CONFLICT: &str = "identity-email-conflict";
    pub const EXTERNAL_ID_CONFLICT: &str = "identity-external-id-conflict";
    pub const DELETE_CONFLICT: &str = "identity-deleted-conflict";
    pub const MISSING_EMAIL: &str = "identity-missing-email";
}

#[derive(Debug, ThisError)]
pub enum IdentityError {
    #[error("User id already taken")]
    UserIdConflict,
    #[error("Name already taken")]
    NameConflict,
    #[error("Name is too long")]
    NameTooLong,
    #[error("Email already used by a user")]
    EmailConflict,
    #[error("External id already linked to a user")]
    ExternalIdConflict,
    #[error("User has no valid email address")]
    MissingEmail,
    #[error("Failed to generate token")]
    TokenConflict,
    #[error("Fingerprint is missing for the requested token kind")]
    TokenMissingFingerprint,
    #[error("Email address is missing for the requested token kind")]
    TokenMissingEmail,
    #[error("User was removed during the operation")]
    UserDeleted,

    #[error(transparent)]
    IdEncoder(#[from] IdEncoderError),
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    DataProtectionError(#[from] DataProtectionError),
}

impl From<IdentityError> for Problem {
    fn from(err: IdentityError) -> Self {
        match err {
            IdentityError::UserIdConflict => Problem::conflict(pr::ID_CONFLICT).with_detail(err),
            IdentityError::NameConflict => Problem::conflict(pr::NAME_CONFLICT).with_detail(err),
            IdentityError::NameTooLong => Problem::bad_request(pr::NAME_TOO_LONG).with_detail(err),
            IdentityError::EmailConflict => Problem::conflict(pr::EMAIL_CONFLICT).with_detail(err),
            IdentityError::ExternalIdConflict => Problem::conflict(pr::EXTERNAL_ID_CONFLICT).with_detail(err),
            IdentityError::MissingEmail => Problem::precondition_failed(pr::MISSING_EMAIL).with_detail(err),
            IdentityError::UserDeleted => Problem::conflict(pr::DELETE_CONFLICT).with_detail(err),
            IdentityError::DBError(err) => err.into(),
            err => Problem::internal_error().with_detail(&err).with_sensitive_dbg(err),
        }
    }
}
