use crate::auth::{AuthBuildError, OIDCConfig};
use async_once_cell::OnceCell;
use oauth2::{reqwest::async_http_client, ClientId, ClientSecret, RedirectUrl, Scope};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata},
    IssuerUrl,
};
use serde::Serialize;
use std::sync::Arc;
use thiserror::Error as ThisError;
use url::Url;

struct ClientInfo {
    client_id: ClientId,
    client_secret: ClientSecret,
    discovery_url: IssuerUrl,
    redirect_url: RedirectUrl,
}

#[derive(ThisError, Debug, Serialize)]
#[error("OpenId Connect discovery failed")]
#[serde(rename_all = "camelCase")]
pub struct OIDCDiscoveryError(pub String);

pub(in crate::auth) struct OIDCClient {
    pub provider: String,
    pub scopes: Vec<Scope>,
    client_info: ClientInfo,
    client: Arc<OnceCell<CoreClient>>,
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

        let client = Self {
            provider: provider.to_string(),
            scopes: config.scopes.iter().map(|scope| Scope::new(scope.clone())).collect(),
            client_info: ClientInfo {
                client_id,
                client_secret,
                discovery_url,
                redirect_url,
            },
            client: Arc::new(OnceCell::new()),
        };

        if let Err(err) = client.client().await {
            if ignore_discovery_error {
                log::warn!("Discovery failed for {provider}: {err}");
            } else {
                return Err(AuthBuildError::OIDCDiscovery(err));
            }
        }

        Ok(Some(client))
    }

    pub async fn client(&self) -> Result<&CoreClient, OIDCDiscoveryError> {
        let client_info = &self.client_info;
        self.client
            .get_or_try_init(async {
                let provider_metadata =
                    match CoreProviderMetadata::discover_async(client_info.discovery_url.clone(), async_http_client)
                        .await
                    {
                        Ok(meta) => meta,
                        Err(err) => return Err(OIDCDiscoveryError(format!("{err}"))),
                    };
                Ok(CoreClient::from_provider_metadata(
                    provider_metadata,
                    client_info.client_id.clone(),
                    Some(client_info.client_secret.clone()),
                )
                .set_redirect_uri(client_info.redirect_url.clone()))
            })
            .await
    }
}
