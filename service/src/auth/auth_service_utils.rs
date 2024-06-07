use crate::{
    auth::{
        auth_session::TokenCookie, token::TokenGenerator, AuthServiceState, AuthSession, OIDCDiscoveryError,
        TokenGeneratorError,
    },
    repositories::{AutoNameError, ExternalUserInfo, Identity, IdentityError, TokenKind},
};
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::Duration;
use serde::Serialize;
use shine_service::{
    axum::{InputError, SiteInfo},
    service::ClientFingerprint,
};
use std::fmt;
use thiserror::Error as ThisError;
use url::Url;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub(in crate::auth) enum UserCreateError {
    #[error("Retry limit reach for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    AutoNameError(#[from] AutoNameError),
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
                None => self.auto_name_manager().generate_name().await?,
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
        kind: TokenKind,
        time_to_live: &Duration,
        fingerprint: Option<&ClientFingerprint>,
        site_info: &SiteInfo,
    ) -> Result<TokenCookie, TokenCreateError> {
        const MAX_RETRY_COUNT: usize = 10;
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new token for user {user_id}, retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(TokenCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let token = TokenGenerator::new(self.random()).generate()?;
            match self
                .identity_manager()
                .add_token(user_id, kind, &token, time_to_live, fingerprint, site_info)
                .await
            {
                Ok(info) => {
                    return Ok(TokenCookie {
                        user_id,
                        key: token,
                        expire_at: info.expire_at,
                        revoked_token: None,
                    })
                }
                Err(IdentityError::TokenConflict) => continue,
                Err(err) => return Err(TokenCreateError::IdentityError(err)),
            }
        }
    }
}

#[derive(Debug, ThisError, Serialize)]
pub(in crate::auth) enum AuthError {
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

        let (kind, status) = match response {
            AuthError::InputError(_) => ("invalidInput", StatusCode::BAD_REQUEST),
            AuthError::InvalidAuthorizationHeader => ("authError", StatusCode::BAD_REQUEST),
            AuthError::LogoutRequired => ("logoutRequired", StatusCode::BAD_REQUEST),
            AuthError::LoginRequired => ("loginRequired", StatusCode::UNAUTHORIZED),
            AuthError::MissingExternalLoginCookie => ("authError", StatusCode::BAD_REQUEST),
            AuthError::MissingNonce => ("authError", StatusCode::BAD_REQUEST),
            AuthError::InvalidCSRF => ("authError", StatusCode::BAD_REQUEST),
            AuthError::TokenExchangeFailed(_) => ("authError", StatusCode::INTERNAL_SERVER_ERROR),
            AuthError::FailedExternalUserInfo(_) => ("authError", StatusCode::BAD_REQUEST),
            AuthError::InvalidToken => ("authError", StatusCode::BAD_REQUEST),
            AuthError::TokenExpired => ("tokenExpired", StatusCode::UNAUTHORIZED),
            AuthError::SessionExpired => ("sessionExpired", StatusCode::UNAUTHORIZED),
            AuthError::InternalServerError(_) => ("internalError", StatusCode::INTERNAL_SERVER_ERROR),
            AuthError::OIDCDiscovery(_) => ("authError", StatusCode::INTERNAL_SERVER_ERROR),
            AuthError::ProviderAlreadyUsed => ("providerAlreadyUsed", StatusCode::CONFLICT),
            AuthError::EmailAlreadyUsed => ("emailAlreadyUsed", StatusCode::CONFLICT),
            AuthError::MissingPrecondition => ("preconditionFailed", StatusCode::PRECONDITION_FAILED),
        };

        let mut target = target_url.unwrap_or(self.error_url()).clone();
        target
            .query_pairs_mut()
            .append_pair("type", kind)
            .append_pair("status", &status.as_u16().to_string());

        let mut context = tera::Context::new();
        context.insert("timeout", &self.page_redirect_time());
        context.insert("redirectUrl", target.as_str());
        context.insert("statusCode", &status.as_u16());
        context.insert("type", kind);
        if self.page_error_detail() {
            let detail = serde_json::to_string(&response).unwrap();
            context.insert("detail", &detail);
        } else {
            context.insert("detail", "");
        }
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
        context.insert("title", self.app_name());
        context.insert("target", target);
        context.insert("redirectUrl", redirect_url.unwrap_or(self.home_url()).as_str());
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
