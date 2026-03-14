use crate::{
    app_state::AppState,
    handlers::{AuthPage, AuthPageHandler, UserSessionHandler},
    models::{ExternalUserInfo, IdentityError, TokenKind},
    repositories::{identity::IdentityDb, session::SessionDb},
    routes::auth::{AuthError, AuthSession, TokenCookie},
    services::{CreateUserError, LinkService, SettingsService, TokenService, UserService},
};
use shine_infra::web::extracts::{ClientFingerprint, SiteInfo};
use url::Url;

/// Handler for external authentication operations (OAuth2/OIDC)
///
/// Orchestrates external login and link flows
pub struct ExternalLoginHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    page_handler: AuthPageHandler<'a>,
    user_session_handler: UserSessionHandler<'a, IDB, SDB>,
    settings: &'a SettingsService,
    token_service: &'a TokenService<IDB>,
    user_service: &'a UserService<IDB>,
    link_service: &'a LinkService<IDB>,
}

impl<'a, IDB, SDB> ExternalLoginHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    /// Create a new external login handler
    pub fn new(
        page_handler: AuthPageHandler<'a>,
        user_session_handler: UserSessionHandler<'a, IDB, SDB>,
        settings: &'a SettingsService,
        token_service: &'a TokenService<IDB>,
        user_service: &'a UserService<IDB>,
        link_service: &'a LinkService<IDB>,
    ) -> Self {
        Self {
            page_handler,
            user_session_handler,
            settings,
            token_service,
            user_service,
            link_service,
        }
    }

    /// Complete external link flow
    ///
    /// Links an external provider account to an existing authenticated user.
    /// Expects user_session to be present in auth_session.
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
        match self.link_service.add_external_link(user.user_id, external_user).await {
            Ok(()) => {}
            Err(IdentityError::ExternalIdConflict) => {
                return self
                    .page_handler
                    .error(auth_session, AuthError::ProviderAlreadyUsed, error_url)
            }
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
        };

        log::debug!(
            "User {} linked to: {}({})",
            user.user_id,
            external_user.provider,
            external_user.provider_id
        );
        let response_session = auth_session.with_external_login(None);
        assert!(response_session.user_session().is_some());
        self.page_handler.redirect(response_session, redirect_url, None)
    }

    /// Complete external login flow
    ///
    /// Handles both registration (new user) and login (existing linked user).
    /// Creates access token if create_token is true, and establishes user session.
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
            .link_service
            .find_by_external_link(external_user.provider.as_str(), external_user.provider_id.as_str())
            .await
        {
            // Found an existing (linked) account
            Ok(Some(identity)) => identity,
            // Create a new (linked) user
            Ok(None) => match self
                .user_service
                .create_linked_user(external_user.name.as_deref(), external_user)
                .await
            {
                Ok(identity) => identity,
                Err(CreateUserError::IdentityError(IdentityError::EmailConflict)) => {
                    return self
                        .page_handler
                        .error(auth_session, AuthError::EmailAlreadyUsed, error_url)
                }
                Err(err) => return self.page_handler.error(auth_session, err, error_url),
            },
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
        };

        // create a new remember me token
        let user_token = if create_token {
            match self
                .token_service
                .create_with_retry(
                    identity.id,
                    TokenKind::Access,
                    &self.settings.token.ttl_access_token,
                    Some(&fingerprint),
                    None,
                    site_info,
                )
                .await
            {
                Ok((token, info)) => Some((identity.id, token, info.expire_at)),
                Err(err) => return self.page_handler.error(auth_session, err, error_url),
            }
        } else {
            None
        };

        let user_session = match self
            .user_session_handler
            .create_user_session(&identity, &fingerprint, site_info)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::warn!("User {} has been deleted during link", identity.id);
                return self
                    .page_handler
                    .error(auth_session.with_access(None), IdentityError::UserDeleted, error_url);
            }
            Err(err) => return self.page_handler.error(auth_session, err, error_url),
        };

        let response_session = auth_session
            .with_external_login(None)
            .with_access(user_token.map(|(user_id, token, expire_at)| TokenCookie {
                user_id,
                key: token,
                expire_at,
                revoked_token: None,
            }))
            .with_session(Some(user_session));
        self.page_handler.redirect(response_session, redirect_url, None)
    }
}

impl AppState {
    pub fn external_login_handler(&self) -> ExternalLoginHandler<'_, impl IdentityDb, impl SessionDb> {
        ExternalLoginHandler::new(
            self.auth_page_handler(),
            self.user_session_handler(),
            self.settings(),
            self.token_service(),
            self.user_service(),
            self.link_service(),
        )
    }
}
