use crate::app_config::{ExternalUserInfoExtensions, OAuth2Config};
use anyhow::Error as AnyError;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, RedirectUrl, Scope, TokenUrl,
};
use openidconnect::UserInfoUrl;
use reqwest::Client as HttpClient;
use std::collections::HashMap;
use url::Url;

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
    pub async fn new(provider: &str, auth_base_url: &Url, config: &OAuth2Config) -> Result<Self, AnyError> {
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
            scopes: config.scopes.iter().map(|scope| Scope::new(scope.clone())).collect(),
            user_info_url,
            user_info_mapping: config.user_info_mapping.clone(),
            extensions: config.extensions.iter().cloned().collect(),
            http_client,
            client,
        })
    }
}
