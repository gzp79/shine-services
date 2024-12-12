use serde::Serialize;
use shine_service::axum::InputError;
use thiserror::Error as ThisError;

use crate::repositories::CaptchaError;

#[derive(Debug, ThisError, Serialize)]
pub enum AuthError {
    #[error("Input validation error")]
    InputError(InputError),
    #[error("Authorization header is malformed")]
    InvalidAuthorizationHeader,
    #[error("Logout required")]
    LogoutRequired,
    #[error("Login required")]
    LoginRequired,
    #[error("Missing external login")]
    MissingExternalLoginCookie,
    #[error("Missing Nonce")]
    MissingNonce,
    #[error("Invalid CSRF state")]
    InvalidCSRF,
    #[error("Failed to exchange authentication token")]
    TokenExchangeFailed(String),
    #[error("Failed to get user info from provider")]
    FailedExternalUserInfo(String),
    #[error("Login token is invalid")]
    InvalidToken,
    #[error("Login token has been revoked")]
    TokenExpired,
    #[error("User session has expired")]
    SessionExpired,
    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Captcha test failed")]
    Captcha(String),
    #[error("Failed to validate captcha")]
    CaptchaServiceError(String),

    // #[error("OpenId discovery failed")]
    // OIDCDiscovery(OIDCDiscoveryError),

    #[error("External provider has already been linked to another user already")]
    ProviderAlreadyUsed,
    #[error("Email has already been linked to another user already")]
    EmailAlreadyUsed,
    #[error("Missing some protection gate to perform operation")]
    MissingPrecondition,
}
