use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession},
    services::SettingsService,
};
use shine_core::web::{Problem, ProblemConfig};
use tera::Tera;
use url::Url;

pub struct PageUtils<'a> {
    settings: &'a SettingsService,
    problem_config: &'a ProblemConfig,
    tera: &'a Tera,
}

impl<'a> PageUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self {
            settings: app_state.settings(),
            problem_config: app_state.problem_config(),
            tera: app_state.tera(),
        }
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

    pub fn error<E>(&self, auth_session: AuthSession, error: E, target_url: Option<&Url>) -> AuthPage
    where
        E: Into<AuthError>,
    {
        let error = error.into();
        log::error!("Page Error: {error:#?}");

        let problem: Problem = self.problem_config.transform(error);
        log::debug!("Page Problem: {problem:#?}");
        let problem_json = serde_json::to_string_pretty(&problem).unwrap();

        let mut target = target_url.unwrap_or(&self.settings.error_url).to_owned();
        target
            .query_pairs_mut()
            .append_pair("type", problem.ty)
            .append_pair("status", &problem.status.as_u16().to_string());

        let mut context = tera::Context::new();
        self.bind_timeout(&mut context);
        context.insert("redirectUrl", target.as_str());
        context.insert("statusCode", &problem.status.as_u16());
        context.insert("type", &problem.ty);
        context.insert("detail", &problem.detail);
        context.insert("problem", &problem_json);

        let html = self
            .tera
            .render("ooops.html", &context)
            .expect("Failed to generate ooops.html template");

        AuthPage {
            auth_session: Some(auth_session),
            html,
        }
    }

    pub fn redirect(&self, auth_session: AuthSession, target: Option<&str>, redirect_url: Option<&Url>) -> AuthPage {
        let mut context = tera::Context::new();
        self.bind_timeout(&mut context);
        self.bind_app_nme(&mut context);
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
