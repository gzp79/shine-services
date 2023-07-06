use std::collections::HashMap;

use crate::{
    auth::{create_redirect_page, AuthServiceState},
    db::{
        CreateIdentityError, DBError, DBSessionError, ExternalLoginInfo, FindIdentity, LinkIdentityError,
        NameGeneratorError,
    },
};
use axum::response::Html;
use reqwest::header;
use serde_json::Value as JsonValue;
use shine_service::service::{CurrentUser, APP_NAME};
use thiserror::Error as ThisError;
use url::Url;
use uuid::Uuid;

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
}

pub(in crate::auth) async fn get_external_user_info(
    url: Url,
    token: &str,
    id_mapping: &HashMap<String, String>,
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
    log::info!("{:?}", user_info);

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

    Ok(ExternalUserInfo {
        external_id,
        name,
        email,
    })
}

#[derive(Debug, ThisError)]
pub(in crate::auth) enum ExternalAuthError {
    #[error("Session or external login cookie is compromised: {0}")]
    CompromisedSessions(String),
    #[error("Email already used by an user")]
    EmailConflict,
    #[error("Provider already linked to an user")]
    ProviderConflict,
    #[error("Number of optimistic concurrency failure limit reached")]
    RetryLimitReached,
    #[error(transparent)]
    NameGeneratorError(#[from] NameGeneratorError),
    #[error("Failed to create session")]
    DBSessionError(#[from] DBSessionError),
    #[error(transparent)]
    DBError(#[from] DBError),
}

pub(in crate::auth) async fn external_auth_user(
    state: &AuthServiceState,
    current_user: Option<CurrentUser>,
    linked_user: Option<CurrentUser>,
    provider: &str,
    user_info: ExternalUserInfo,
    target_url: Option<&str>,
) -> Result<(CurrentUser, Html<String>), ExternalAuthError> {
    let external_login = ExternalLoginInfo {
        provider: provider.to_string(),
        provider_id: user_info.external_id,
    };

    if let Some(linked_user) = linked_user {
        // Link the current user to an external provider
        let current_user = current_user.ok_or(ExternalAuthError::CompromisedSessions(
            "Missing user session to link".to_string(),
        ))?;
        if current_user.user_id != linked_user.user_id || current_user.key != linked_user.key {
            return Err(ExternalAuthError::CompromisedSessions(
                "External login is not matching the user session during linking".to_string(),
            ));
        }

        match state
            .identity_manager()
            .link_user(current_user.user_id, &external_login)
            .await
        {
            Ok(()) => {}
            Err(LinkIdentityError::LinkProviderConflict) => return Err(ExternalAuthError::ProviderConflict),
            Err(LinkIdentityError::DBError(err)) => return Err(ExternalAuthError::DBError(err)),
        };

        log::debug!("Link ready: {current_user:#?}");
        let html = create_redirect_page(state, "Redirecting", APP_NAME, target_url);
        Ok((current_user, html))
    } else {
        log::debug!("Finding existing user by external login...");
        let identity = match state
            .identity_manager()
            .find(FindIdentity::ExternalLogin(&external_login))
            .await?
        {
            Some(identity) => {
                log::debug!("Found: {identity:#?}");
                // Sign in to an existing (linked) account
                identity
            }
            None => {
                // Create a new user.
                const MAX_RETRY_COUNT: usize = 10;
                let mut retry_count = 0;
                loop {
                    log::debug!("Creating new user; retry: {retry_count:#?}");
                    if retry_count > MAX_RETRY_COUNT {
                        return Err(ExternalAuthError::RetryLimitReached);
                    }

                    let user_id = Uuid::new_v4();
                    let user_name = match &user_info.name {
                        Some(name) if retry_count == 0 => name.clone(),
                        _ => state.name_generator().generate_name().await?,
                    };
                    let email = user_info.email.as_deref();
                    retry_count += 1;

                    use CreateIdentityError::*;
                    match state
                        .identity_manager()
                        .create_user(user_id, &user_name, email, Some(&external_login))
                        .await
                    {
                        Ok(identity) => break identity,
                        Err(NameConflict) => continue,
                        Err(UserIdConflict) => continue,
                        Err(LinkEmailConflict) => return Err(ExternalAuthError::EmailConflict),
                        Err(LinkProviderConflict) => return Err(ExternalAuthError::ProviderConflict),
                        Err(DBError(err)) => return Err(ExternalAuthError::DBError(err)),
                    };
                }
            }
        };

        log::debug!("Identity ready: {identity:#?}");
        let current_user = state.session_manager().create(&identity).await?;
        let html = create_redirect_page(state, "Redirecting", APP_NAME, target_url);
        Ok((current_user, html))
    }
}
