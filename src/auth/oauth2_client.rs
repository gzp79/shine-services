use std::collections::HashMap;

use crate::auth::{AuthBuildError, OAuth2Config};
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, Scope, TokenUrl};
use openidconnect::UserInfoUrl;

pub(in crate::auth) struct OAuth2Client {
    pub provider: String,
    pub scopes: Vec<Scope>,
    pub user_info_url: UserInfoUrl,
    pub user_info_mapping: HashMap<String, String>,
    pub client: BasicClient,
}

impl OAuth2Client {
    pub async fn new(provider: &str, config: &OAuth2Config) -> Result<Self, AuthBuildError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string())
            .map_err(|err| AuthBuildError::RedirectUrl(format!("{err}")))?;
        let auth_url = AuthUrl::new(config.authorization_url.clone())
            .map_err(|err| AuthBuildError::InvalidAuthUrl(format!("{err}")))?;
        let token_url =
            TokenUrl::new(config.token_url.clone()).map_err(|err| AuthBuildError::InvalidTokenUrl(format!("{err}")))?;
        let user_info_url = UserInfoUrl::new(config.user_info_url.clone())
            .map_err(|err| AuthBuildError::InvalidUserInfoUrl(format!("{err}")))?;
        let client =
            BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url)).set_redirect_uri(redirect_url);

        Ok(Self {
            provider: provider.to_string(),
            scopes: config.scopes.iter().map(|scope| Scope::new(scope.clone())).collect(),
            user_info_url,
            user_info_mapping: config.user_info_mapping.clone(),
            client,
        })
    }
}
