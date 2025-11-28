use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, PageUtils, TokenCookie},
    handlers::CreateUserError,
    repositories::identity::{ExternalUserInfo, IdentityError, TokenKind},
};
use shine_infra::web::extracts::{ClientFingerprint, InputError, SiteInfo, ValidationErrorEx};
use url::Url;
use validator::ValidationError;

pub struct AuthUtils<'a> {
    state: &'a AppState,
}

impl<'a> AuthUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { state: app_state }
    }

    pub fn validate_redirect_url(&self, property: &'static str, redirect_url: &Url) -> Result<(), InputError> {
        if self
            .state
            .settings()
            .allowed_redirect_urls
            .iter()
            .any(|r| r.is_match(redirect_url.as_str()))
        {
            Ok(())
        } else {
            Err(ValidationError::new("invalid-redirect-url")
                .with_message("Redirect URL is not allowed".into())
                .into_constraint_error(property))
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
                return PageUtils::new(self.state).error(
                    auth_session,
                    AuthError::ProviderAlreadyUsed,
                    error_url,
                    redirect_url,
                )
            }
            Err(err) => return PageUtils::new(self.state).error(auth_session, err, error_url, redirect_url),
        };

        log::debug!(
            "User {} linked to: {}({})",
            user.user_id,
            external_user.provider,
            external_user.provider_id
        );
        let response_session = auth_session.with_external_login(None);
        assert!(response_session.user_session().is_some());
        PageUtils::new(self.state).redirect(response_session, redirect_url, None)
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
                    return PageUtils::new(self.state).error(
                        auth_session,
                        AuthError::EmailAlreadyUsed,
                        error_url,
                        redirect_url,
                    )
                }
                Err(err) => return PageUtils::new(self.state).error(auth_session, err, error_url, redirect_url),
            },
            Err(err) => return PageUtils::new(self.state).error(auth_session, err, error_url, redirect_url),
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
                Err(err) => return PageUtils::new(self.state).error(auth_session, err, error_url, redirect_url),
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
                return PageUtils::new(self.state).error(
                    auth_session.with_access(None),
                    IdentityError::UserDeleted,
                    error_url,
                    redirect_url,
                );
            }
            Err(err) => return PageUtils::new(self.state).error(auth_session, err, error_url, redirect_url),
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
        PageUtils::new(self.state).redirect(response_session, redirect_url, None)
    }
}
