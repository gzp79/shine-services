use crate::auth::{AuthServiceState, AuthSession};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shine_service::{axum::Page, service::APP_NAME};
use url::Url;

pub(in crate::auth) struct AuthPage {
    auth_session: Option<AuthSession>,
    status: StatusCode,
    html: String,
}

impl AuthPage {
    /// Return a redirect page to some url of the application.
    pub fn redirect(state: &AuthServiceState, auth_session: Option<AuthSession>, redirect_url: Option<&Url>) -> Self {
        let mut context = tera::Context::new();
        context.insert("title", "Redirecting...");
        context.insert("target", APP_NAME);
        context.insert("redirect_url", redirect_url.unwrap_or(state.home_url()).as_str());
        let html = state
            .tera()
            .render("redirect.html", &context)
            .expect("Failed to generate redirect.html template");

        Self {
            auth_session,
            status: StatusCode::OK,
            html,
        }
    }

    /// Return a redirect page to some external url.
    pub fn external_redirect<S: AsRef<str>>(
        state: &AuthServiceState,
        auth_session: Option<AuthSession>,
        target: S,
        redirect_url: &Url,
    ) -> Self {
        let mut context = tera::Context::new();
        context.insert("title", "Redirecting...");
        context.insert("target", target.as_ref());
        context.insert("redirect_url", redirect_url.as_str());
        let html = state
            .tera()
            .render("redirect.html", &context)
            .expect("Failed to generate redirect.html template");

        Self {
            auth_session,
            status: StatusCode::OK,
            html,
        }
    }

    /// Return an error page and updates the auth cookies.
    pub fn error<S: ToString>(
        state: &AuthServiceState,
        auth_session: Option<AuthSession>,
        status: StatusCode,
        err: S,
    ) -> Self {
        let mut context = tera::Context::new();
        context.insert("home_url", state.home_url());
        context.insert("detail", &err.to_string());
        let html = state
            .tera()
            .render("ooops.html", &context)
            .expect("Failed to generate ooops.html template");

        AuthPage {
            auth_session,
            status,
            html,
        }
    }

    /// Return an internal server error page.
    pub fn internal_error<S: ToString>(state: &AuthServiceState, auth_session: Option<AuthSession>, err: S) -> Self {
        Self::error(state, auth_session, StatusCode::INTERNAL_SERVER_ERROR, err)
    }

    /// Create an error page about invalid cookies and clear them on the client.
    pub fn invalid_session_logout(state: &AuthServiceState, mut auth_session: AuthSession) -> Self {
        let _ = auth_session.take();
        Self::error(
            state,
            Some(auth_session),
            StatusCode::FORBIDDEN,
            "Session expired, clearing user cookies",
        )
    }
}

impl IntoResponse for AuthPage {
    fn into_response(self) -> Response {
        (self.auth_session, Page::new_with_status(self.status, self.html)).into_response()
    }
}
