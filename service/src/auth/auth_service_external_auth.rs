use crate::{
    auth::{
        auth_service_utils::{CreateTokenKind, UserCreateError},
        extensions, AuthError, AuthPage, AuthServiceState, AuthSession, ExternalUserInfoExtensions,
    },
    repositories::{ExternalUserInfo, IdentityError, SiteInfo},
};
use reqwest::{header, Client as HttpClient};
use serde_json::Value as JsonValue;
use shine_service::service::{ClientFingerprint, APP_NAME};
use std::collections::HashMap;
use thiserror::Error as ThisError;
use url::Url;

#[derive(Debug, ThisError)]
pub(in crate::auth) enum ExternalUserInfoError {
    #[error("Error in request: {0}")]
    RequestError(String),
    #[error("Unexpected response: {0}")]
    ResponseError(String),
    #[error("Unexpected response content: {0}")]
    ResponseContentError(String),
    #[error("Cannot find external user id")]
    MissingExternalId,
    #[error("{0:?} failed with: {1}")]
    Extension(ExternalUserInfoExtensions, String),
}

impl AuthServiceState {
    pub(in crate::auth) async fn get_external_user_info(
        &self,
        client: &HttpClient,
        url: Url,
        provider: &str,
        token: &str,
        id_mapping: &HashMap<String, String>,
        extensions: &[ExternalUserInfoExtensions],
    ) -> Result<ExternalUserInfo, ExternalUserInfoError> {
        let response = client
            .get(url)
            .bearer_auth(token)
            .header(header::USER_AGENT, APP_NAME)
            .send()
            .await
            .map_err(|err| ExternalUserInfoError::RequestError(format!("{err}")))?;

        let user_info = if response.status().is_success() {
            response
                .json::<JsonValue>()
                .await
                .map_err(|err| ExternalUserInfoError::ResponseContentError(format!("{err}")))?
        } else {
            return Err(ExternalUserInfoError::ResponseError(format!(
                "({}), {}",
                response.status(),
                response.text().await.unwrap_or_default(),
            )));
        };
        log::info!("external user info: {:?}", user_info);

        let external_id_id = id_mapping.get("id").map(|s| s.as_str()).unwrap_or("id");
        let external_id = user_info
            .get(external_id_id)
            .and_then(|v| match v {
                JsonValue::Number(id) => Some(id.to_string()),
                JsonValue::String(id) => Some(id.to_owned()),
                _ => None,
            })
            .ok_or(ExternalUserInfoError::MissingExternalId)?;
        log::debug!("{external_id_id} - {external_id:?}");

        let name_id = id_mapping.get("name").map(|s| s.as_str()).unwrap_or("name");
        let name = user_info.get(name_id).and_then(|v| v.as_str()).map(ToOwned::to_owned);
        log::debug!("{name_id} - {name:?}");
        let email_id = id_mapping.get("email").map(|s| s.as_str()).unwrap_or("email");
        let email = user_info.get(email_id).and_then(|v| v.as_str()).map(ToOwned::to_owned);
        log::debug!("{email_id} - {email:?}");

        let mut external_user_info = ExternalUserInfo {
            provider: provider.to_string(),
            provider_id: external_id,
            name,
            email,
        };

        log::info!("Checking extensions: {:?}", extensions);
        for extension in extensions {
            match extension {
                ExternalUserInfoExtensions::GithubEmail => {
                    external_user_info = extensions::get_github_user_email(client, external_user_info, token)
                        .await
                        .map_err(|err| ExternalUserInfoError::Extension(*extension, err))?
                }
            };
        }

        Ok(external_user_info)
    }
}

impl AuthServiceState {
    pub(in crate::auth) async fn page_external_link(
        &self,
        auth_session: AuthSession,
        external_user: &ExternalUserInfo,
        target_url: Option<&Url>,
        error_url: Option<&Url>,
    ) -> AuthPage {
        // at this point current user, linked_user, etc. should be consistent due to auth_session construction
        assert!(auth_session.token_login.is_none());

        let user = auth_session.user.clone().unwrap();
        match self.identity_manager().link_user(user.user_id, external_user).await {
            Ok(()) => {}
            Err(IdentityError::LinkProviderConflict) => {
                return self.page_error(auth_session, AuthError::ProviderAlreadyUsed, error_url)
            }
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        log::debug!(
            "User {} linked to: {}({})",
            user.user_id,
            external_user.provider,
            external_user.provider_id
        );
        self.page_redirect(auth_session, APP_NAME, target_url)
    }

    pub(in crate::auth) async fn page_external_login(
        &self,
        mut auth_session: AuthSession,
        fingerprint: ClientFingerprint,
        site_info: &SiteInfo,
        external_user: &ExternalUserInfo,
        target_url: Option<&Url>,
        error_url: Option<&Url>,
        create_token: bool,
    ) -> AuthPage {
        assert!(auth_session.user.is_none());
        assert!(auth_session.token_login.is_none());

        log::debug!("Checking if this is a login or registration...");
        log::debug!("{external_user:#?}");
        let identity = match self
            .identity_manager()
            .find_by_external_link(external_user.provider.as_str(), external_user.provider_id.as_str())
            .await
        {
            // Found an existing (linked) account
            Ok(Some(identity)) => identity,
            // Create a new (linked) user
            Ok(None) => match self.create_user_with_retry(Some(external_user)).await {
                Ok(identity) => identity,
                Err(UserCreateError::IdentityError(IdentityError::LinkEmailConflict)) => {
                    return self.page_error(auth_session, AuthError::EmailAlreadyUsed, error_url)
                }
                Err(err) => return self.page_internal_error(auth_session, err, error_url),
            },
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        // create a new remember me token
        let token_login = if create_token {
            match self
                .create_token_with_retry(identity.id, Some(&fingerprint), site_info, CreateTokenKind::AutoRenewal)
                .await
            {
                Ok(token_login) => Some(token_login),
                Err(err) => return self.page_internal_error(auth_session, err, error_url),
            }
        } else {
            None
        };

        // find roles (for new user it will be an empty list)
        let roles = match self.identity_manager().get_roles(identity.id).await {
            Ok(Some(roles)) => roles,
            Ok(None) => return self.page_internal_error(auth_session, IdentityError::UserDeleted, error_url),
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        log::debug!("Identity created: {identity:#?}");
        let user = match self
            .session_manager()
            .create(&identity, roles, &fingerprint, site_info)
            .await
        {
            Ok(user) => user,
            Err(err) => return self.page_internal_error(auth_session, err, error_url),
        };

        auth_session.token_login = token_login;
        auth_session.user = Some(user);
        self.page_redirect(auth_session, APP_NAME, target_url)
    }
}
