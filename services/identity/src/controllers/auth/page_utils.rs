use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession},
    services::SettingsService,
};
use shine_infra::web::responses::{Problem, ProblemConfig};
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

    pub fn error<E>(
        &self,
        auth_session: AuthSession,
        error: E,
        error_url: Option<&Url>,
        redirect_url: Option<&Url>,
    ) -> AuthPage
    where
        E: Into<AuthError>,
    {
        let error = error.into();
        log::error!("Page Error: {error:#?}");
        let problem: Problem = self.problem_config.transform(error);

        let mut target_url = error_url.unwrap_or(&self.settings.error_url).to_owned();

        {
            let mut query = target_url.query_pairs_mut();

            query
                .clear()
                .append_pair("type", problem.ty)
                .append_pair("status", &problem.status.as_u16().to_string());

            if let Some(redirect_url) = redirect_url {
                query.append_pair("redirectUrl", redirect_url.as_str());
            }
        }

        self.redirect(auth_session, Some(&target_url), Some(&problem))
    }

    pub fn problem(
        &self,
        auth_session: AuthSession,
        problem: Problem,
        error_url: Option<&Url>,
        redirect_url: Option<&Url>,
    ) -> AuthPage {
        log::error!("Page Error: {problem:#?}");
        let problem: Problem = self.problem_config.transform(problem);

        let mut target_url = error_url.unwrap_or(&self.settings.error_url).to_owned();

        {
            let mut query = target_url.query_pairs_mut();

            query
                .clear()
                .append_pair("type", problem.ty)
                .append_pair("status", &problem.status.as_u16().to_string());

            if let Some(redirect_url) = redirect_url {
                query.append_pair("redirectUrl", redirect_url.as_str());
            }
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
