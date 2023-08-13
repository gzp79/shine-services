use crate::auth::{extensions, ExternalUserInfoExtensions};
use reqwest::header;
use serde_json::Value as JsonValue;
use shine_service::service::APP_NAME;
use std::collections::HashMap;
use thiserror::Error as ThisError;
use url::Url;

#[derive(Clone, Debug)]
pub(in crate::auth) struct ExternalUserInfo {
    pub provider: String,
    pub provider_id: String,
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
    provider: &str,
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
                external_user_info = extensions::get_github_user_email(external_user_info, token)
                    .await
                    .map_err(|err| ExternalUserInfoError::Extension(*extension, err))?
            }
        };
    }

    Ok(external_user_info)
}
