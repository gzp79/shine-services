use crate::{
    auth::{auth_session::TokenLogin, AuthServiceState, AuthSession, TokenGeneratorError},
    db::{ExternalLoginInfo, Identity, IdentityError, NameGeneratorError},
};
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use shine_service::service::APP_NAME;
use std::fmt;
use thiserror::Error as ThisError;
use url::Url;
use uuid::Uuid;

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
        mut default_name: Option<&str>,
        email: Option<&str>,
        external_login: Option<&ExternalLoginInfo>,
    ) -> Result<Identity, UserCreateError> {
        const MAX_RETRY_COUNT: usize = 10;
        let mut retry_count = 0;
        loop {
            log::debug!("Creating new user; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(UserCreateError::RetryLimitReached);
            }
            retry_count += 1;

            let user_id = Uuid::new_v4();
            let user_name = match default_name.take() {
                Some(name) => name.to_string(),
                None => self.name_generator().generate_name().await?,
            };

            match self
                .identity_manager()
                .create_user(user_id, &user_name, email, external_login)
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
    pub(in crate::auth) async fn create_token_with_retry(&self, user_id: Uuid) -> Result<TokenLogin, TokenCreateError> {
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
                .create_token(user_id, &token, &self.token().max_duration())
                .await
            {
                Ok(token) => {
                    return Ok(TokenLogin {
                        user_id,
                        token: token.token,
                        expires: token.expire_at,
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

    #[error("External provider has already been linked to another user already")]
    ProviderAlreadyUsed,
    #[error("Email has already been linked to another user already")]
    EmailAlreadyUsed,
}

pub(in crate::auth) struct AuthPage {
    pub status: StatusCode,
    pub auth_session: Option<AuthSession>,
    pub html: String,
}

impl IntoResponse for AuthPage {
    fn into_response(self) -> Response {
        (self.status, self.auth_session, Html(self.html)).into_response()
    }
}

impl AuthServiceState {
    pub(in crate::auth) fn page_error(
        &self,
        auth_session: AuthSession,
        response: AuthError,
        target_url: Option<&Url>,
    ) -> AuthPage {
        let mut context = tera::Context::new();
        context.insert("redirect_url", target_url.unwrap_or(self.home_url()));
        //context.insert("response", &response);
        context.insert("detail", &response.to_string());
        let html = self
            .tera()
            .render("ooops.html", &context)
            .expect("Failed to generate ooops.html template");

        AuthPage {
            status: StatusCode::OK,
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
        context.insert("title", APP_NAME);
        context.insert("target", target);
        context.insert("redirect_url", redirect_url.unwrap_or(self.home_url()).as_str());
        let html = self
            .tera()
            .render("redirect.html", &context)
            .expect("Failed to generate redirect.html template");

        AuthPage {
            status: StatusCode::OK,
            auth_session: Some(auth_session),
            html,
        }
    }
}
