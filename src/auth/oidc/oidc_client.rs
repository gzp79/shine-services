use crate::auth::{AuthBuildError, OIDCConfig};
use oauth2::{reqwest::async_http_client, ClientId, ClientSecret, RedirectUrl, Scope};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata},
    IssuerUrl,
};
use url::Url;

pub(in crate::auth) struct OIDCClient {
    pub provider: String,
    pub scopes: Vec<Scope>,
    pub client: CoreClient,
}

impl OIDCClient {
    pub async fn new(
        provider: &str,
        auth_base_url: &Url,
        config: &OIDCConfig,
        ignore_discovery_error: bool,
    ) -> Result<Option<Self>, AuthBuildError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let redirect_url = auth_base_url
            .join(&format!("{provider}/auth"))
            .map_err(|err| AuthBuildError::RedirectUrl(format!("{err}")))?;
        let redirect_url =
            RedirectUrl::new(redirect_url.to_string()).map_err(|err| AuthBuildError::RedirectUrl(format!("{err}")))?;
        let discovery_url = IssuerUrl::new(config.discovery_url.clone())
            .map_err(|err| AuthBuildError::InvalidIssuer(format!("{err}")))?;
        // todo: For a slightly better solution discovery could be moved into the request as a InitOnce cell, thus
        // a failure of discovery at startup can be ignored without disabling the provider completely.
        let provider_metadata = match CoreProviderMetadata::discover_async(discovery_url, async_http_client).await {
            Ok(meta) => meta,
            Err(err) => match err {
                openidconnect::DiscoveryError::Request(err) if ignore_discovery_error => {
                    log::error!("Discovery failed for: {provider}: {err}");
                    return Ok(None);
                }
                _ => return Err(AuthBuildError::Discovery(format!("{err}"))),
            },
        };
        let client = CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
            .set_redirect_uri(redirect_url);

        Ok(Some(Self {
            provider: provider.to_string(),
            scopes: config.scopes.iter().map(|scope| Scope::new(scope.clone())).collect(),
            client,
        }))
    }
}
