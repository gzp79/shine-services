use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, TokenCookie},
    handlers::CreateUserError,
    repositories::identity::{ExternalUserInfo, IdentityError, TokenKind},
    services::SettingsService,
};
use shine_infra::web::{
    extracts::{ClientFingerprint, InputError, SiteInfo, ValidationError},
    responses::{Problem, ProblemConfig},
};
use tera::Tera;
use url::Url;

pub struct PageUtils<'a> {
    state: &'a AppState,
    settings: &'a SettingsService,
    problem_config: &'a ProblemConfig,
    tera: &'a Tera,
}

impl<'a> PageUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self {
            state: app_state,
            settings: app_state.settings(),
            problem_config: app_state.problem_config(),
            tera: app_state.tera(),
        }
    }

    pub fn validate_redirect_url(&self, redirect_url: Option<&Url>) -> Result<Option<Url>, InputError> {
        let redirect_url = match redirect_url {
            Some(url) => url.clone(),
            None => return Ok(None),
        };

        if self
            .settings
            .allowed_redirect_urls
            .iter()
            .any(|r| r.is_match(redirect_url.as_str()))
        {
            Ok(Some(redirect_url))
        } else {
            let mut error = ValidationError::new_field("redirectUrl", "constraint");
            error.add_param("message", "Invalid redirect url");
            Err(InputError::Constraint(error))
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

    pub async fn complete_external_link(
        &self,
        auth_session: AuthSession,
        external_user: &ExternalUserInfo,
        redirect_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        log::debug!("Completing external link for user: {external_user:#?}");
        assert!(auth_session.user_session().is_some());

        let user = auth_session.user_session().unwrap();
        match self
            .state
            .identity_service()
            .add_external_link(user.user_id, external_user)
            .await
        {
            Ok(()) => {}
            Err(IdentityError::LinkProviderConflict) => {
                return self.error(
                    auth_session,
                    AuthError::ProviderAlreadyUsed,
                    error_url,
                    redirect_url,
                )
            }
            Err(err) => return self.error(auth_session, err, error_url, redirect_url),
        };

        log::debug!(
            "User {} linked to: {}({})",
            user.user_id,
            external_user.provider,
            external_user.provider_id
        );
        let response_session = auth_session.with_external_login(None);
        assert!(response_session.user_session().is_some());
        self.redirect(response_session, redirect_url, None)
    }

    pub async fn complete_external_login(
        &self,
        auth_session: AuthSession,
        fingerprint: ClientFingerprint,
        site_info: &SiteInfo,
        external_user: &ExternalUserInfo,
        redirect_url: Option<&Url>,
        error_url: Option<&Url>,
        create_token: bool,
    ) -> AuthPage {
        log::debug!("Completing external login for user: {external_user:#?}");
        assert!(auth_session.user_session().is_none());
        assert!(auth_session.access().is_none());

        log::debug!("Checking if this is a login or registration...");
        log::debug!("{external_user:#?}");
        let identity = match self
            .state
            .identity_service()
            .find_by_external_link(external_user.provider.as_str(), external_user.provider_id.as_str())
            .await
        {
            // Found an existing (linked) account
            Ok(Some(identity)) => identity,
            // Create a new (linked) user
            Ok(None) => match self
                .state
                .create_user_service()
                .create_user(
                    Some(external_user),
                    external_user.name.as_deref(),
                    external_user.email.as_deref(),
                )
                .await
            {
                Ok(identity) => identity,
                Err(CreateUserError::IdentityError(IdentityError::EmailConflict)) => {
                    return self.error(
                        auth_session,
                        AuthError::EmailAlreadyUsed,
                        error_url,
                        redirect_url,
                    )
                }
                Err(err) => return self.error(auth_session, err, error_url, redirect_url),
            },
            Err(err) => return self.error(auth_session, err, error_url, redirect_url),
        };

        // create a new remember me token
        let user_token = if create_token {
            match self
                .state
                .login_token_handler()
                .create_user_token(
                    identity.id,
                    TokenKind::Access,
                    &self.state.settings().token.ttl_access_token,
                    Some(&fingerprint),
                    site_info,
                )
                .await
            {
                Ok(token_cookie) => Some(token_cookie),
                Err(err) => return self.error(auth_session, err, error_url, redirect_url),
            }
        } else {
            None
        };

        let user_session = match self
            .state
            .user_info_handler()
            .create_user_session(&identity, &fingerprint, site_info)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::warn!("User {} has been deleted during link", identity.id);
                return self.error(
                    auth_session.with_access(None),
                    IdentityError::UserDeleted,
                    error_url,
                    redirect_url,
                );
            }
            Err(err) => return self.error(auth_session, err, error_url, redirect_url),
        };

        let response_session = auth_session
            .with_external_login(None)
            .with_access(user_token.map(|user_token| TokenCookie {
                user_id: user_token.user_id,
                key: user_token.token,
                expire_at: user_token.expire_at,
                revoked_token: None,
            }))
            .with_session(Some(user_session));
        self.redirect(response_session, redirect_url, None)
    }
}
