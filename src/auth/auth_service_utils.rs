use crate::{
    auth::{auth_session::TokenLogin, AuthServiceState, AuthSession, TokenGeneratorError},
    db::{ExternalUserInfo, Identity, IdentityError, NameGeneratorError, TokenKind},
};
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::Duration;
use shine_service::{
    axum::ValidationError,
    service::{ClientFingerprint, APP_NAME},
};
use std::fmt;
use thiserror::Error as ThisError;
use url::Url;
use uuid::Uuid;

use super::oidc::OIDCDiscoveryError;

#[derive(Debug, ThisError)]
pub(in crate::auth) enum UserCreateError {
    #[error("Retry limit reach for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    NameGeneratorError(#[from] NameGeneratorError),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl AuthServiceState {
    pub(in crate::auth) async fn create_user_with_retry(
        &self,
        external_user: Option<&ExternalUserInfo>,
    ) -> Result<Identity, UserCreateError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut default_name = external_user.as_ref().and_then(|u| u.name.clone());
        let email = external_user.as_ref().and_then(|u| u.email.as_deref());
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new user; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(UserCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let user_id = Uuid::new_v4();
            let user_name = match default_name.take() {
                Some(name) => name,
                None => self.name_generator().generate_name().await?,
            };

            match self
                .identity_manager()
                .create_user(user_id, &user_name, email, external_user)
                .await
            {
                Ok(identity) => return Ok(identity),
                Err(IdentityError::NameConflict) => continue,
                Err(IdentityError::UserIdConflict) => continue,
                Err(err) => return Err(UserCreateError::IdentityError(err)),
            }
        }
    }
}

pub(in crate::auth) enum CreateTokenKind {
    SingleAccess,
    Persistent(Duration),
    AutoRenewal,
}

#[derive(Debug, ThisError)]
pub(in crate::auth) enum TokenCreateError {
    #[error("Retry limit reach for token creation")]
    RetryLimitReached,
    #[error("Failed to generate token: {0}")]
    TokenGenerateError(#[from] TokenGeneratorError),
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl AuthServiceState {
    // Create a new login token for the given user.
    pub(in crate::auth) async fn create_token_with_retry(
        &self,
        user_id: Uuid,
        fingerprint: Option<&ClientFingerprint>,
        kind: CreateTokenKind,
    ) -> Result<TokenLogin, TokenCreateError> {
        let (duration, kind) = match kind {
            CreateTokenKind::SingleAccess => (self.token().ttl_single_access(), TokenKind::SingleAccess),
            CreateTokenKind::AutoRenewal => (self.token().ttl_remember_me(), TokenKind::AutoRenewal),
            CreateTokenKind::Persistent(duration) => (duration, TokenKind::Persistent),
        };
        let fingerprint = fingerprint.map(|f| f.to_compact_string());

        const MAX_RETRY_COUNT: usize = 10;
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new token for user {user_id}, retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(TokenCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let token = self.token().generate_token()?;
            match self
                .identity_manager()
                .create_token(user_id, &token, &duration, fingerprint.as_deref(), kind)
                .await
            {
                Ok(token) => {
                    return Ok(TokenLogin {
                        user_id,
                        token: token.token,
                        expires: token.expire,
                    })
                }
                Err(IdentityError::TokenConflict) => continue,
                Err(err) => return Err(TokenCreateError::IdentityError(err)),
            }
        }
    }
}

#[derive(Debug, ThisError)]
pub(in crate::auth) enum AuthError {
    #[error("Input validation error")]
    ValidationError(ValidationError),
    #[error("Logout required")]
    LogoutRequired,
    #[error("Login required")]
    LoginRequired,
    #[error("Missing external login")]
    MissingExternalLogin,
    #[error("Missing Nonce")]
    MissingNonce,
    #[error("Invalid CSRF state")]
    InvalidCSRF,
    #[error("Failed to exchange authentication token")]
    TokenExchangeFailed(String),
    #[error("Failed to get user info from provider")]
    FailedExternalUserInfo,
    #[error("Login token is invalid")]
    TokenInvalid,
    #[error("Login token has been revoked")]
    TokenExpired,
    #[error("User session has expired")]
    SessionExpired,
    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("OpenId discovery failed")]
    OIDCDiscovery(OIDCDiscoveryError),

    #[error("External provider has already been linked to another user already")]
    ProviderAlreadyUsed,
    #[error("Email has already been linked to another user already")]
    EmailAlreadyUsed,
    #[error("Missing some protection gate to perform operation")]
    MissingPrecondition,
}

pub(in crate::auth) struct AuthPage {
    pub auth_session: Option<AuthSession>,
    pub html: String,
}

impl IntoResponse for AuthPage {
    fn into_response(self) -> Response {
        (self.auth_session, Html(self.html)).into_response()
    }
}

impl AuthServiceState {
    pub(in crate::auth) fn page_error(
        &self,
        auth_session: AuthSession,
        response: AuthError,
        target_url: Option<&Url>,
    ) -> AuthPage {
        log::error!("{response:?}");

        //todo: give some more detail and add a few more error sources
        let (kind, status, detail) = match response {
            AuthError::ValidationError(_) => ("invalidInput", StatusCode::BAD_REQUEST, String::new()),
            AuthError::LogoutRequired => ("logoutRequired", StatusCode::BAD_REQUEST, String::new()),
            AuthError::LoginRequired => ("loginRequired", StatusCode::UNAUTHORIZED, String::new()),
            AuthError::MissingExternalLogin => ("authError", StatusCode::BAD_REQUEST, String::new()),
            AuthError::MissingNonce => ("authError", StatusCode::BAD_REQUEST, String::new()),
            AuthError::InvalidCSRF => ("authError", StatusCode::BAD_REQUEST, String::new()),
            AuthError::TokenExchangeFailed(err) => ("authError", StatusCode::INTERNAL_SERVER_ERROR, err),
            AuthError::FailedExternalUserInfo => ("authError", StatusCode::BAD_REQUEST, String::new()),
            AuthError::TokenInvalid => ("authError", StatusCode::BAD_REQUEST, String::new()),
            AuthError::TokenExpired => ("sessionExpired", StatusCode::UNAUTHORIZED, String::new()),
            AuthError::SessionExpired => ("sessionExpired", StatusCode::UNAUTHORIZED, String::new()),
            AuthError::InternalServerError(_) => ("internalError", StatusCode::INTERNAL_SERVER_ERROR, String::new()),
            AuthError::OIDCDiscovery(err) => ("authError", StatusCode::INTERNAL_SERVER_ERROR, err.0),
            AuthError::ProviderAlreadyUsed => ("providerAlreadyUsed", StatusCode::CONFLICT, String::new()),
            AuthError::EmailAlreadyUsed => ("emailAlreadyUsed", StatusCode::CONFLICT, String::new()),
            AuthError::MissingPrecondition => ("preconditionFailed", StatusCode::PRECONDITION_FAILED, String::new()),
        };

        let mut target = target_url.unwrap_or(self.error_url()).clone();
        target
            .query_pairs_mut()
            .append_pair("type", kind)
            .append_pair("status", &status.as_u16().to_string());

        let mut context = tera::Context::new();
        context.insert("timeout", &self.page_redirect_time());
        context.insert("redirect_url", target.as_str());
        context.insert("statusCode", &status.as_u16());
        context.insert("type", kind);
        context.insert("detail", &detail);
        let html = self
            .tera()
            .render("ooops.html", &context)
            .expect("Failed to generate ooops.html template");

        AuthPage {
            auth_session: Some(auth_session),
            html,
        }
    }

    pub(in crate::auth) fn page_internal_error<E: fmt::Debug>(
        &self,
        auth_session: AuthSession,
        err: E,
        target_url: Option<&Url>,
    ) -> AuthPage {
        self.page_error(
            auth_session,
            AuthError::InternalServerError(format!("{err:?}")),
            target_url,
        )
    }

    pub(in crate::auth) fn page_redirect(
        &self,
        auth_session: AuthSession,
        target: &str,
        redirect_url: Option<&Url>,
    ) -> AuthPage {
        let mut context = tera::Context::new();
        context.insert("timeout", &self.page_redirect_time());
        context.insert("title", APP_NAME);
        context.insert("target", target);
        context.insert("redirect_url", redirect_url.unwrap_or(self.home_url()).as_str());
        let html = self
            .tera()
            .render("redirect.html", &context)
            .expect("Failed to generate redirect.html template");

        AuthPage {
            auth_session: Some(auth_session),
            html,
        }
    }
}
