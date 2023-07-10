use crate::{
    auth::{github_ext, AuthPage, AuthServiceState, AuthSession, ExternalUserInfoExtensions},
    db::{ExternalLoginInfo, FindIdentity, IdentityError},
};
use axum::http::StatusCode;
use reqwest::header;
use serde_json::Value as JsonValue;
use shine_service::service::{CurrentUser, APP_NAME};
use std::collections::HashMap;
use thiserror::Error as ThisError;
use url::Url;

#[derive(Clone, Debug)]
pub(in crate::auth) struct ExternalUserInfo {
    pub external_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

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

pub(in crate::auth) async fn get_external_user_info(
    url: Url,
    token: &str,
    id_mapping: &HashMap<String, String>,
    extensions: &[ExternalUserInfoExtensions],
) -> Result<ExternalUserInfo, ExternalUserInfoError> {
    let client = reqwest::Client::new();
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

    let name_id = id_mapping.get("name").map(|s| s.as_str()).unwrap_or("name");
    let name = user_info.get(name_id).and_then(|v| v.as_str()).map(ToOwned::to_owned);
    let email_id = id_mapping.get("email").map(|s| s.as_str()).unwrap_or("email");
    let email = user_info.get(email_id).and_then(|v| v.as_str()).map(ToOwned::to_owned);

    let mut external_user_info = ExternalUserInfo {
        external_id,
        name,
        email,
    };

    log::info!("Checking extensions: {:?}", extensions);
    for extension in extensions {
        match extension {
            ExternalUserInfoExtensions::GithubEmail => {
                external_user_info = github_ext::get_github_user_email(external_user_info, token)
                    .await
                    .map_err(|err| ExternalUserInfoError::Extension(*extension, err))?
            }
        };
    }

    Ok(external_user_info)
}

pub(in crate::auth) async fn page_external_auth(
    state: &AuthServiceState,
    mut auth_session: AuthSession,
    linked_user: Option<CurrentUser>,
    provider: &str,
    user_info: ExternalUserInfo,
    target_url: Option<Url>,
) -> AuthPage {
    // Make sure external login is cleared
    let _ = auth_session.external_login.take();

    if let Some(linked_user) = linked_user {
        log::debug!("Link the current user to an external provider...");

        // check is there is a current user
        let user = match auth_session.user.as_ref() {
            Some(user) => user,
            None => return AuthPage::invalid_session_logout(state, auth_session),
        };

        // check if linked and current users are matching
        if user.user_id != linked_user.user_id || user.key != linked_user.key {
            return AuthPage::invalid_session_logout(state, auth_session);
        }

        let external_login = ExternalLoginInfo {
            provider: provider.to_string(),
            provider_id: user_info.external_id.clone(),
        };
        match state.identity_manager().link_user(user.user_id, &external_login).await {
            Ok(()) => {}
            Err(IdentityError::LinkProviderConflict) => {
                return AuthPage::error(
                    state,
                    Some(auth_session),
                    StatusCode::CONFLICT,
                    "Provider already linked",
                )
            }
            Err(IdentityError::DBError(err)) => return AuthPage::internal_error(state, Some(auth_session), err),
            Err(err) => {
                return AuthPage::internal_error(state, Some(auth_session), format!("Unexpected error: {err:?}"))
            }
        };

        log::debug!("Linked user: {user:#?}");
        return AuthPage::redirect(state, Some(auth_session), target_url.as_ref());
    } else {
        log::debug!("Login in or register a new user...");

        // Check if there is no current user.
        if auth_session.user.is_some() {
            return AuthPage::invalid_session_logout(state, auth_session);
        }

        log::debug!("Check for existing user by external login...");
        let external_login = ExternalLoginInfo {
            provider: provider.to_string(),
            provider_id: user_info.external_id.clone(),
        };
        let identity = match state
            .identity_manager()
            .find(FindIdentity::ExternalLogin(&external_login))
            .await
        {
            Ok(identity) => identity,
            Err(err) => return AuthPage::internal_error(state, Some(auth_session), err),
        };
        let identity = match identity {
            // Sign in to an existing (linked) account
            Some(identity) => identity,

            // Create a new user.
            None => match state
                .create_user_with_retry(
                    user_info.name.as_deref(),
                    user_info.email.as_deref(),
                    Some(&external_login),
                )
                .await
            {
                Ok(identity) => identity,
                Err(err) => return AuthPage::internal_error(state, Some(auth_session), err),
            },
        };

        log::debug!("Identity created: {identity:#?}");
        let user = match state.session_manager().create(&identity).await {
            Ok(user) => user,
            Err(err) => return AuthPage::internal_error(state, Some(auth_session), err),
        };

        auth_session.user = Some(user);
        AuthPage::redirect(state, Some(auth_session), target_url.as_ref())
    }
}
