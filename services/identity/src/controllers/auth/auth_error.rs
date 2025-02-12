use reqwest::StatusCode;
use serde::Serialize;
use shine_core::web::{InputError, IntoProblem, Problem, ProblemConfig};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
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
    TokenExchangeFailed { error: String },
    #[error("Failed to get user info from provider")]
    FailedExternalUserInfo { error: String },
    #[error("Login token is invalid")]
    InvalidToken,
    #[error("Login token has been revoked")]
    TokenExpired,
    #[error("User session has expired")]
    SessionExpired,
    #[error("Internal server error: {error}")]
    InternalServerError { error: String },

    #[error("Captcha test failed")]
    Captcha { error: String },
    #[error("Failed to validate captcha")]
    CaptchaServiceError { error: String },

    #[error("OpenId discovery failed")]
    OIDCDiscovery { error: String },

    #[error("External provider has already been linked to another user already")]
    ProviderAlreadyUsed,
    #[error("Email has already been linked to another user already")]
    EmailAlreadyUsed,
    #[error("Missing some protection gate to perform operation")]
    MissingPrecondition,
}

impl IntoProblem for AuthError {
    fn into_problem(self, config: &ProblemConfig) -> Problem {
        let problem = match &self {
            AuthError::InputError(_) => Problem::new(StatusCode::BAD_REQUEST, "invalidInput"),
            AuthError::InvalidAuthorizationHeader => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::LogoutRequired => Problem::new(StatusCode::BAD_REQUEST, "logoutRequired"),
            AuthError::LoginRequired => Problem::new(StatusCode::UNAUTHORIZED, "loginRequired"),
            AuthError::MissingExternalLoginCookie => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::MissingNonce => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::InvalidCSRF => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::TokenExchangeFailed { .. } => Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "authError"),
            AuthError::FailedExternalUserInfo { .. } => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::InvalidToken => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::TokenExpired => Problem::new(StatusCode::UNAUTHORIZED, "tokenExpired"),
            AuthError::SessionExpired => Problem::new(StatusCode::UNAUTHORIZED, "sessionExpired"),
            AuthError::InternalServerError { .. } => Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "internalError"),
            AuthError::Captcha { .. } => Problem::new(StatusCode::BAD_REQUEST, "authError"),
            AuthError::CaptchaServiceError { .. } => Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "authError"),
            AuthError::OIDCDiscovery { .. } => Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "authError"),
            AuthError::ProviderAlreadyUsed => Problem::new(StatusCode::CONFLICT, "providerAlreadyUsed"),
            AuthError::EmailAlreadyUsed => Problem::new(StatusCode::CONFLICT, "emailAlreadyUsed"),
            AuthError::MissingPrecondition => Problem::new(StatusCode::PRECONDITION_FAILED, "preconditionFailed"),
        };

        if config.include_internal {
            problem.with_detail(format!("{}", self)).with_extension(self)
        } else {
            problem.with_detail(format!("{}", self))
        }
    }
}
