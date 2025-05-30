use crate::{
    app_config::{ExternalUserInfoExtensions, OAuth2Config},
    repositories::identity::ExternalUserInfo,
};
use anyhow::Error as AnyError;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, RedirectUrl,
    Scope, TokenUrl,
};
use openidconnect::UserInfoUrl;
use reqwest::{header, Client as HttpClient};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use thiserror::Error as ThisError;
use url::Url;
use validator::ValidateEmail;

#[derive(Debug, ThisError)]
pub enum OAuth2Error {
    #[error("Error in request: {0}")]
    RequestError(String),
    #[error("Unexpected response: {0}")]
    ResponseError(String),
    #[error("Unexpected response content: {0}")]
    ResponseContentError(String),
    #[error("Cannot find external user id")]
    MissingExternalId,
}

type CoreClient<
    HasAuthUrl = EndpointSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointSet,
> = BasicClient<HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasRevocationUrl, HasTokenUrl>;

pub struct OAuth2Client {
    pub provider: String,
    pub scopes: Vec<Scope>,
    pub user_info_url: UserInfoUrl,
    pub user_info_mapping: HashMap<String, String>,
    pub extensions: Vec<ExternalUserInfoExtensions>,
    pub http_client: HttpClient,
    pub client: CoreClient,
}

impl OAuth2Client {
    pub async fn new(
        provider: &str,
        auth_base_url: &Url,
        config: &OAuth2Config,
    ) -> Result<Self, AnyError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let redirect_url = auth_base_url.join(&format!("{provider}/auth"))?;
        let redirect_url = RedirectUrl::new(redirect_url.to_string())?;
        let auth_url = AuthUrl::new(config.authorization_url.clone())?;
        let token_url = TokenUrl::new(config.token_url.clone())?;
        let user_info_url = UserInfoUrl::new(config.user_info_url.clone())?;
        let client = BasicClient::new(client_id)
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url)
            .set_client_secret(client_secret);

        let ignore_certificates = config.ignore_certificates.unwrap_or(false);
        let http_client = HttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .danger_accept_invalid_certs(ignore_certificates)
            .build()?;

        Ok(Self {
            provider: provider.to_string(),
            scopes: config
                .scopes
                .iter()
                .map(|scope| Scope::new(scope.clone()))
                .collect(),
            user_info_url,
            user_info_mapping: config.user_info_mapping.clone(),
            extensions: config.extensions.iter().cloned().collect(),
            http_client,
            client,
        })
    }

    pub async fn get_external_user_info(
        &self,
        app_name: &str,
        url: Url,
        provider: &str,
        token: &str,
        id_mapping: &HashMap<String, String>,
        extensions: &[ExternalUserInfoExtensions],
    ) -> Result<ExternalUserInfo, OAuth2Error> {
        let client = &self.http_client;

        let response = client
            .get(url)
            .bearer_auth(token)
            .header(header::USER_AGENT, app_name)
            .send()
            .await
            .map_err(|err| OAuth2Error::RequestError(format!("{err}")))?;

        let user_info = if response.status().is_success() {
            response
                .json::<JsonValue>()
                .await
                .map_err(|err| OAuth2Error::ResponseContentError(format!("{err}")))?
        } else {
            return Err(OAuth2Error::ResponseError(format!(
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
            .ok_or(OAuth2Error::MissingExternalId)?;
        log::debug!("{external_id_id} - {external_id:?}");

        let name_id = id_mapping.get("name").map(|s| s.as_str()).unwrap_or("name");
        let name = user_info
            .get(name_id)
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        log::debug!("{name_id} - {name:?}");
        let email_id = id_mapping
            .get("email")
            .map(|s| s.as_str())
            .unwrap_or("email");
        let email = user_info
            .get(email_id)
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
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
                    external_user_info = self
                        .get_github_user_email(external_user_info, app_name, token)
                        .await?
                }
            };
        }

        Ok(external_user_info)
    }

    async fn get_github_user_email(
        &self,
        mut external_user_info: ExternalUserInfo,
        app_name: &str,
        token: &str,
    ) -> Result<ExternalUserInfo, OAuth2Error> {
        if external_user_info.email.is_none() {
            let client = &self.http_client;

            let url = Url::parse("https://api.github.com/user/emails").unwrap();
            let response = client
                .get(url)
                .bearer_auth(token)
                .header(header::USER_AGENT, app_name)
                .send()
                .await
                .map_err(|err| OAuth2Error::RequestError(format!("{err}")))?;

            #[derive(Deserialize, Debug)]
            struct Email {
                email: String,
                primary: bool,
            }

            let email_info = if response.status().is_success() {
                response
                    .json::<Vec<Email>>()
                    .await
                    .map_err(|err| OAuth2Error::ResponseContentError(format!("{err}")))?
            } else {
                return Err(OAuth2Error::ResponseError(format!(
                    "({}), {}",
                    response.status(),
                    response.text().await.unwrap_or_default(),
                )));
            };
            log::info!("{:?}", email_info);

            external_user_info.email = email_info
                .into_iter()
                .find(|email| email.primary)
                .map(|email| email.email)
                .filter(|email| email.validate_email());
        }

        Ok(external_user_info)
    }
}
