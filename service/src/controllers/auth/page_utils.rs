use crate::controllers::{
    auth::{AuthError, AuthPage, AuthSession},
    AppSettings, AppState,
};
use axum::http::StatusCode;
use std::fmt;
use tera::Tera;
use url::Url;

pub struct PageUtils<'a> {
    settings: &'a AppSettings,
    tera: &'a Tera,
}

impl<'a> PageUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self {
            settings: app_state.settings(),
            tera: app_state.tera(),
        }
    }

    pub fn error(&self, auth_session: AuthSession, response: AuthError, target_url: Option<&Url>) -> AuthPage {
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
            AuthError::Captcha(_) => ("authError", StatusCode::BAD_REQUEST),
            AuthError::CaptchaServiceError(_) => ("authError", StatusCode::INTERNAL_SERVER_ERROR),
            //AuthError::OIDCDiscovery(_) => ("authError", StatusCode::INTERNAL_SERVER_ERROR),
            AuthError::ProviderAlreadyUsed => ("providerAlreadyUsed", StatusCode::CONFLICT),
            AuthError::EmailAlreadyUsed => ("emailAlreadyUsed", StatusCode::CONFLICT),
            AuthError::MissingPrecondition => ("preconditionFailed", StatusCode::PRECONDITION_FAILED),
        };

        let mut target = target_url.unwrap_or(&self.settings.error_url).to_owned();
        target
            .query_pairs_mut()
            .append_pair("type", kind)
            .append_pair("status", &status.as_u16().to_string());

        let mut context = tera::Context::new();
        context.insert("timeout", &self.settings.page_redirect_time);
        context.insert("redirectUrl", target.as_str());
        context.insert("statusCode", &status.as_u16());
        context.insert("type", kind);
        if self.settings.full_problem_response {
            let detail = serde_json::to_string(&response).unwrap();
            context.insert("detail", &detail);
        } else {
            context.insert("detail", "");
        }
        let html = self
            .tera
            .render("ooops.html", &context)
            .expect("Failed to generate ooops.html template");

        AuthPage {
            auth_session: Some(auth_session),
            html,
        }
    }

    pub fn internal_error<E: fmt::Debug>(
        &self,
        auth_session: AuthSession,
        err: E,
        target_url: Option<&Url>,
    ) -> AuthPage {
        self.error(
            auth_session,
            AuthError::InternalServerError(format!("{err:?}")),
            target_url,
        )
    }

    pub fn redirect(&self, auth_session: AuthSession, target: Option<&str>, redirect_url: Option<&Url>) -> AuthPage {
        let mut context = tera::Context::new();
        context.insert("timeout", &self.settings.page_redirect_time);
        context.insert("title", &self.settings.app_name);
        context.insert("target", target.unwrap_or(&self.settings.app_name));
        context.insert("redirectUrl", redirect_url.unwrap_or(&self.settings.home_url).as_str());
        let html = self
            .tera
            .render("redirect.html", &context)
            .expect("Failed to generate redirect.html template");

        AuthPage {
            auth_session: Some(auth_session),
            html,
        }
    }
}
