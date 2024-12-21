use crate::{
    controllers::{
        auth::{AuthError, AuthPage, AuthSession, PageUtils, TokenCookie},
        AppState,
    },
    repositories::identity::{ExternalUserInfo, IdentityError, TokenKind},
    services::UserCreateError,
};
use shine_core::{
    axum::SiteInfo,
    service::{ClientFingerprint, CurrentUser},
};
use url::Url;

pub struct LinkUtils<'a> {
    state: &'a AppState,
}

impl<'a> LinkUtils<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { state: app_state }
    }

    pub async fn complete_external_link(
        &self,
        auth_session: AuthSession,
        external_user: &ExternalUserInfo,
        target_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        // at this point current user, linked_user, etc. should be consistent due to auth_session construction
        assert!(auth_session.token_cookie.is_none());

        let user = auth_session.user_session.clone().unwrap();
        match self
            .state
            .identity_service()
            .add_external_link(user.user_id, external_user)
            .await
        {
            Ok(()) => {}
            Err(IdentityError::LinkProviderConflict) => {
                return PageUtils::new(self.state).error(auth_session, AuthError::ProviderAlreadyUsed, error_url)
            }
            Err(err) => return PageUtils::new(self.state).internal_error(auth_session, err, error_url),
        };

        log::debug!(
            "User {} linked to: {}({})",
            user.user_id,
            external_user.provider,
            external_user.provider_id
        );
        PageUtils::new(self.state).redirect(auth_session, None, target_url)
    }

    pub async fn complete_external_login(
        &self,
        mut auth_session: AuthSession,
        fingerprint: ClientFingerprint,
        site_info: &SiteInfo,
        external_user: &ExternalUserInfo,
        target_url: Option<&Url>,
        error_url: Option<&Url>,
        create_token: bool,
    ) -> AuthPage {
        assert!(auth_session.user_session.is_none());
        assert!(auth_session.token_cookie.is_none());

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
            Ok(None) => match self.state.create_user_service().create_user(Some(external_user)).await {
                Ok(identity) => identity,
                Err(UserCreateError::IdentityError(IdentityError::LinkEmailConflict)) => {
                    return PageUtils::new(self.state).error(auth_session, AuthError::EmailAlreadyUsed, error_url)
                }
                Err(err) => return PageUtils::new(self.state).internal_error(auth_session, err, error_url),
            },
            Err(err) => return PageUtils::new(self.state).internal_error(auth_session, err, error_url),
        };

        // create a new remember me token
        let user_token = if create_token {
            match self
                .state
                .token_service()
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
                Err(err) => return PageUtils::new(self.state).internal_error(auth_session, err, error_url),
            }
        } else {
            None
        };

        // find roles (for new user it will be an empty list)
        let roles = match self.state.identity_service().get_roles(identity.id).await {
            Ok(Some(roles)) => roles,
            Ok(None) => {
                return PageUtils::new(self.state).internal_error(auth_session, IdentityError::UserDeleted, error_url)
            }
            Err(err) => return PageUtils::new(self.state).internal_error(auth_session, err, error_url),
        };

        log::debug!("Identity created: {identity:#?}");
        let user_session = match self
            .state
            .session_service()
            .create(&identity, roles, &fingerprint, site_info)
            .await
        {
            Ok(user) => user,
            Err(err) => return PageUtils::new(self.state).internal_error(auth_session, err, error_url),
        };

        auth_session.token_cookie = user_token.map(|user_token| TokenCookie {
            user_id: user_token.user_id,
            key: user_token.token,
            expire_at: user_token.expire_at,
            revoked_token: None,
        });
        auth_session.user_session = Some(CurrentUser {
            user_id: user_session.0.info.user_id,
            key: user_session.1,
            session_start: user_session.0.info.created_at,
            name: user_session.0.user.name,
            roles: user_session.0.user.roles,
            fingerprint: user_session.0.info.fingerprint,
            version: user_session.0.user_version,
        });
        PageUtils::new(self.state).redirect(auth_session, None, target_url)
    }
}
