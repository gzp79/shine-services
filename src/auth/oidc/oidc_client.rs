use crate::auth::{AuthBuildError, OIDCConfig};
use oauth2::{reqwest::async_http_client, ClientId, ClientSecret, RedirectUrl, Scope};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata},
    IssuerUrl,
};

pub(in crate::auth) struct OIDCClient {
    pub provider: String,
    pub scopes: Vec<Scope>,
    pub client: CoreClient,
}

impl OIDCClient {
    pub async fn new(provider: &str, config: &OIDCConfig) -> Result<Self, AuthBuildError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string())
            .map_err(|err| AuthBuildError::RedirectUrl(format!("{err}")))?;
        let discovery_url = IssuerUrl::new(config.discovery_url.clone())
            .map_err(|err| AuthBuildError::InvalidIssuer(format!("{err}")))?;
        let provider_metadata = CoreProviderMetadata::discover_async(discovery_url, async_http_client)
            .await
            .map_err(|err| AuthBuildError::Discovery(format!("{err}")))?;
        let client = CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
            .set_redirect_uri(redirect_url);

        log::info!(
            "Redirect url for provider {}: {:?}",
            provider,
            config.redirect_url.as_str()
        );

        Ok(Self {
            provider: provider.to_string(),
            scopes: config.scopes.iter().map(|scope| Scope::new(scope.clone())).collect(),
            client,
        })
    }
}
