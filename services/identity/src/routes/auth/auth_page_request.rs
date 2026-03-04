use crate::{
    app_state::AppState,
    routes::auth::{AuthPage, AuthSession, AuthUtils, PageUtils},
};
use shine_infra::web::{
    extracts::{InputError, ValidatedQuery},
    responses::ErrorResponse,
};
use url::Url;

/// Helper for common auth page request handling patterns
/// Uses builder pattern with early-return error handling
pub struct AuthPageRequest<'a> {
    state: &'a AppState,
    auth_session: AuthSession,
}

impl<'a> AuthPageRequest<'a> {
    pub fn new(state: &'a AppState, auth_session: AuthSession) -> Self {
        Self { state, auth_session }
    }

    /// Validate query result (handles query parsing errors)
    /// Returns Ok(query) on success, Err(AuthPage) for early return on error
    pub fn validate_query<T>(&self, query: Result<ValidatedQuery<T>, ErrorResponse<InputError>>) -> Result<T, AuthPage>
    where
        T: serde::de::DeserializeOwned + validator::Validate,
    {
        match query {
            Ok(ValidatedQuery(query)) => Ok(query),
            Err(error) => Err(PageUtils::new(self.state).error(self.auth_session.clone(), error.problem, None)),
        }
    }

    /// Validate redirect URLs (both error_url and redirect_url)
    /// Returns None on success, Some(AuthPage) for early return on error
    pub fn validate_redirect_urls(&self, redirect_url: Option<&Url>, error_url: Option<&Url>) -> Option<AuthPage> {
        // Validate error_url first (no error_url to report errors to)
        if let Some(error_url) = error_url {
            if let Err(err) = AuthUtils::new(self.state).validate_redirect_url("errorUrl", error_url) {
                return Some(PageUtils::new(self.state).error(self.auth_session.clone(), err, None));
            }
        }

        // Validate redirect_url (can report errors to error_url)
        if let Some(redirect_url) = redirect_url {
            if let Err(err) = AuthUtils::new(self.state).validate_redirect_url("redirectUrl", redirect_url) {
                return Some(PageUtils::new(self.state).error(self.auth_session.clone(), err, error_url));
            }
        }

        None
    }

    /// Validate captcha
    /// Returns None on success, Some(AuthPage) for early return on error
    pub async fn validate_captcha(&self, captcha: Option<&str>, error_url: Option<&Url>) -> Option<AuthPage> {
        if let Err(err) = self.state.captcha_validator().validate(captcha).await {
            return Some(PageUtils::new(self.state).error(self.auth_session.clone(), err, error_url));
        }
        None
    }

    /// Clear auth state (revoke both access token and session)
    pub async fn clear_auth_state(mut self) -> Self {
        self.auth_session = self
            .auth_session
            .with_external_login(None)
            .revoke_access(self.state)
            .await
            .revoke_session(self.state)
            .await;
        self
    }

    /// Get auth session reference
    pub fn auth_session(&self) -> &AuthSession {
        &self.auth_session
    }

    /// Consume and return auth session
    pub fn into_auth_session(self) -> AuthSession {
        self.auth_session
    }

    /// Get state reference
    pub fn state(&self) -> &AppState {
        self.state
    }

    /// Error page helper
    pub fn error_page<E>(self, error: E, error_url: Option<&Url>) -> AuthPage
    where
        E: Into<crate::routes::auth::AuthError>,
    {
        PageUtils::new(self.state).error(self.auth_session, error, error_url)
    }
}
