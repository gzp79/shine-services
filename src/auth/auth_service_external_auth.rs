use crate::{
    auth::{auth_service_utils::UserCreateError, AuthError, AuthPage, AuthServiceState, AuthSession, ExternalUserInfo},
    db::{ExternalLoginInfo, FindIdentity, IdentityError},
};
use shine_service::service::APP_NAME;
use url::Url;

impl AuthServiceState {
    pub(in crate::auth) async fn page_external_link(
        &self,
        auth_session: AuthSession,
        provider: &str,
        provider_id: &str,
        target_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        // at this point current user, linked_user, etc. should be consistent due to auth_session construction
        assert!(auth_session.token_login.is_none());

        let external_login = ExternalLoginInfo {
            provider: provider.to_string(),
            provider_id: provider_id.to_string(),
        };

        let user = auth_session.user.clone().unwrap();
        match self.identity_manager().link_user(user.user_id, &external_login).await {
            Ok(()) => {}
            Err(IdentityError::LinkProviderConflict) => {
                return self.page_error(auth_session, AuthError::ProviderAlreadyUsed, error_url)
            }
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        log::debug!("User {} linked to: {}", user.user_id, provider);
        self.page_redirect(auth_session, APP_NAME, target_url)
    }

    pub(in crate::auth) async fn page_external_login(
        &self,
        mut auth_session: AuthSession,
        external_user_info: ExternalUserInfo,
        target_url: Option<&Url>,
        error_url: Option<&Url>,
        create_token: bool,
    ) -> AuthPage {
        assert!(auth_session.user.is_none());
        assert!(auth_session.token_login.is_none());

        let external_login = ExternalLoginInfo {
            provider: external_user_info.provider.clone(),
            provider_id: external_user_info.provider_id.clone(),
        };

        log::debug!("Checking if this is a login or registration...");
        let identity = match self
            .identity_manager()
            .find(FindIdentity::ExternalLogin(&external_login))
            .await
        {
            // Found an existing (linked) account
            Ok(Some(identity)) => identity,
            // Create a new (linked) user
            Ok(None) => {
                match self
                    .create_user_with_retry(
                        external_user_info.name.as_deref(),
                        external_user_info.email.as_deref(),
                        Some(&external_login),
                    )
                    .await
                {
                    Ok(identity) => identity,
                    Err(UserCreateError::IdentityError(IdentityError::LinkEmailConflict)) => {
                        return self.page_error(auth_session, AuthError::EmailAlreadyUsed, error_url)
                    }
                    Err(err) => return self.page_internal_error(auth_session, err, error_url),
                }
            }
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        // create a new token
        let token_login = if create_token {
            match self.create_token_with_retry(identity.user_id).await {
                Ok(token_login) => Some(token_login),
                Err(err) => return self.page_internal_error(auth_session, err, error_url),
            }
        } else {
            None
        };

        // find roles (for new user it will be an empty list)
        let roles = match self.identity_manager().get_roles(identity.user_id).await {
            Ok(roles) => roles,
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        log::debug!("Identity created: {identity:#?}");
        let user = match self.session_manager().create(&identity, roles).await {
            Ok(user) => user,
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        auth_session.token_login = token_login;
        auth_session.user = Some(user);
        self.page_redirect(auth_session, APP_NAME, target_url)
    }
}
