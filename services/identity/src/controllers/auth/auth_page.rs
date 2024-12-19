use axum::response::{Html, IntoResponse, Response};

use super::AuthSession;

pub struct AuthPage {
    pub auth_session: Option<AuthSession>,
    pub html: String,
}

impl IntoResponse for AuthPage {
    fn into_response(self) -> Response {
        (self.auth_session, Html(self.html)).into_response()
    }
}
