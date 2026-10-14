use crate::{
    app_state::AppState,
    routes::auth::{AuthError, AuthSession},
    settings::IdentitySettings,
};
use axum::response::{Html, IntoResponse, Response};
use shine_infra::web::responses::{Problem, ProblemConfig};
use tera::Tera;
use url::Url;

pub struct AuthPage {
    pub auth_session: Option<AuthSession>,
    pub html: String,
}

impl IntoResponse for AuthPage {
    fn into_response(self) -> Response {
        (self.auth_session, Html(self.html)).into_response()
    }
}

pub struct AuthPageHandler<'a> {
    settings: &'a IdentitySettings,
    problem_config: &'a ProblemConfig,
    tera: &'a Tera,
}

impl<'a> AuthPageHandler<'a> {
    pub fn new(settings: &'a IdentitySettings, problem_config: &'a ProblemConfig, tera: &'a Tera) -> Self {
        Self { settings, problem_config, tera }
    }

    fn bind_app_nme(&self, context: &mut tera::Context) {
        context.insert("appName", &self.settings.app_name);
    }

    fn bind_timeout(&self, context: &mut tera::Context) {
        if let Some(page_redirect_time) = self.settings.page_redirect_time {
            context.insert("timeout", &page_redirect_time);
        } else {
            context.insert("timeout", &-1);
        }
    }

    pub fn error<E>(&self, auth_session: AuthSession, error: E, error_url: Option<&Url>) -> AuthPage
    where
        E: Into<AuthError>,
    {
        let error = error.into();
        let error_text = error.to_string();
        let problem: Problem = self.problem_config.transform(error);

        // Client errors (4xx) are expected in normal use, only server errors are real errors.
        // Only the error message is logged, the debug dump may contain user data.
        if problem.status.is_server_error() {
            log::error!("Page Error: {error_text} ({}, {})", problem.status, problem.ty);
        } else {
            log::warn!("Page Error: {error_text} ({}, {})", problem.status, problem.ty);
        }

        let mut target_url = error_url.unwrap_or(&self.settings.error_url).to_owned();

        {
            let mut query = target_url.query_pairs_mut();
            query.append_pair("errorType", problem.ty);
        }

        self.redirect(auth_session, Some(&target_url), Some(&problem))
    }

    pub fn redirect(&self, auth_session: AuthSession, target_url: Option<&Url>, problem: Option<&Problem>) -> AuthPage {
        let mut context = tera::Context::new();

        self.bind_timeout(&mut context);
        self.bind_app_nme(&mut context);

        context.insert("targetUrl", target_url.unwrap_or(&self.settings.home_url).as_str());

        let problem_json = problem
            .map(|p| serde_json::to_string_pretty(&p).unwrap())
            .unwrap_or_default();
        context.insert("problem", &problem_json);

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

impl AppState {
    pub fn auth_page_handler(&self) -> AuthPageHandler<'_> {
        AuthPageHandler::new(self.settings(), self.problem_config(), self.tera())
    }
}
