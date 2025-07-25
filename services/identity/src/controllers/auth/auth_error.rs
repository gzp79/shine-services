use crate::{
    handlers::{CreateUserError, LoginEmailError, LoginTokenError, UserInfoError},
    repositories::{identity::IdentityError, session::SessionError, CaptchaError},
};
use reqwest::StatusCode;
use shine_infra::web::{extracts::InputError, responses::Problem};
use thiserror::Error as ThisError;

const INPUT_ERROR: &str = "auth-input-error";
const AUTH_ERROR: &str = "auth-error";
const LOGIN_REQUIRED: &str = "auth-login-required";
const TOKEN_EXPIRED: &str = "auth-token-expired";
const SESSION_EXPIRED: &str = "auth-session-expired";
const EMAIL_CONFLICT: &str = "auth-register-email-conflict";
const EXTERNAL_ID_CONFLICT: &str = "auth-register-external-id-conflict";
const MISSING_CONFIRMATION: &str = "auth-not-confirmed";
const EMAIL_LOGIN: &str = "auth-email-login";

const EXTERNAL_MISSING_COOKIE: &str = "external-missing-cookie";
const EXTERNAL_INVALID_NONCE: &str = "external-invalid-nonce";
const EXTERNAL_INVALID_CSRF: &str = "external-invalid-csrf";
const EXTERNAL_EXCHANGE_FAILED: &str = "external-exchange-failed";
const EXTERNAL_INFO_FAILED: &str = "external-info-failed";
const EXTERNAL_DISCOVERY_FAILED: &str = "external-discovery-failed";

#[derive(Debug, ThisError)]
pub enum ExternalLoginError {
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
    #[error("OpenId discovery failed")]
    OIDCDiscovery(String),
}

impl From<ExternalLoginError> for Problem {
    fn from(value: ExternalLoginError) -> Self {
        match value {
            ExternalLoginError::MissingExternalLoginCookie => Problem::bad_request(EXTERNAL_MISSING_COOKIE),
            ExternalLoginError::MissingNonce => Problem::bad_request(EXTERNAL_INVALID_NONCE),
            ExternalLoginError::InvalidCSRF => Problem::bad_request(EXTERNAL_INVALID_CSRF),
            ExternalLoginError::TokenExchangeFailed(error) => {
                Problem::internal_error_ty(EXTERNAL_EXCHANGE_FAILED).with_sensitive(error)
            }
            ExternalLoginError::FailedExternalUserInfo(error) => {
                Problem::internal_error_ty(EXTERNAL_INFO_FAILED).with_sensitive(error)
            }
            ExternalLoginError::OIDCDiscovery(error) => {
                Problem::internal_error_ty(EXTERNAL_DISCOVERY_FAILED).with_sensitive(error)
            }
        }
    }
}

#[derive(Debug, ThisError)]
pub enum AuthError {
    #[error("Authorization header is malformed")]
    InvalidHeader,
    #[error("Login required")]
    LoginRequired,
    #[error("Login token is invalid")]
    InvalidToken,
    #[error("Login token has been revoked")]
    TokenExpired,
    #[error("Email has been altered")]
    EmailConflict,
    #[error("User session has expired")]
    SessionExpired,
    #[error("External provider has already been linked to another user already")]
    ProviderAlreadyUsed,
    #[error("Email has already been linked to another user already")]
    EmailAlreadyUsed,
    #[error("Missing operation confirmation")]
    MissingConfirmation,
    #[error("Waiting email login flow")]
    EmailLogin,

    #[error(transparent)]
    InputError(#[from] InputError),
    #[error(transparent)]
    CaptchaError(#[from] CaptchaError),
    #[error(transparent)]
    SessionError(#[from] SessionError),
    #[error(transparent)]
    ExternalLoginError(#[from] ExternalLoginError),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
    #[error(transparent)]
    CreateUserError(#[from] CreateUserError),
    #[error(transparent)]
    LoginTokenError(#[from] LoginTokenError),
    #[error(transparent)]
    LoginEmailError(#[from] LoginEmailError),
    #[error(transparent)]
    UserInfoError(#[from] UserInfoError),

    #[error("Internal server error")]
    InternalServerError(Problem),
}

impl From<AuthError> for Problem {
    fn from(value: AuthError) -> Self {
        match value {
            AuthError::LoginRequired => Problem::unauthorized_ty(LOGIN_REQUIRED),
            AuthError::InvalidHeader => Problem::unauthorized_ty(TOKEN_EXPIRED).with_sensitive("invalidHeader"),
            AuthError::TokenExpired => Problem::unauthorized_ty(TOKEN_EXPIRED).with_sensitive("expiredToken"),
            AuthError::InvalidToken => Problem::unauthorized_ty(TOKEN_EXPIRED).with_sensitive("invalidToken"),
            AuthError::EmailConflict => Problem::unauthorized_ty(TOKEN_EXPIRED).with_sensitive("emailConflict"),
            AuthError::SessionExpired => Problem::unauthorized_ty(SESSION_EXPIRED),
            AuthError::ProviderAlreadyUsed => Problem::conflict(EXTERNAL_ID_CONFLICT),
            AuthError::EmailAlreadyUsed => Problem::conflict(EMAIL_CONFLICT),
            AuthError::MissingConfirmation => Problem::conflict(MISSING_CONFIRMATION),
            AuthError::EmailLogin => Problem::new(StatusCode::ACCEPTED, EMAIL_LOGIN),

            AuthError::InputError(input_error) => {
                Problem::bad_request(INPUT_ERROR).with_sensitive(Problem::from(input_error))
            }
            AuthError::CaptchaError(error @ CaptchaError::MissingCaptcha) => {
                Problem::bad_request(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::CaptchaError(error @ CaptchaError::FailedValidation(_)) => {
                Problem::bad_request(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::CaptchaError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::SessionError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::IdentityError(IdentityError::UserDeleted) => {
                Problem::unauthorized_ty(SESSION_EXPIRED).with_sensitive("userDeleted")
            }
            AuthError::IdentityError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::ExternalLoginError(error) => {
                let problem: Problem = error.into();
                Problem::new(problem.status, AUTH_ERROR).with_sensitive(problem)
            }
            AuthError::CreateUserError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::LoginTokenError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::LoginEmailError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::UserInfoError(error) => {
                Problem::internal_error_ty(AUTH_ERROR).with_sensitive(Problem::from(error))
            }
            AuthError::InternalServerError(error) => Problem::internal_error_ty(AUTH_ERROR).with_sensitive(error),
        }
    }
}
