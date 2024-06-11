use crate::auth::{async_http_client, AuthBuildError, OIDCConfig};
use log::info;
use oauth2::{reqwest::AsyncHttpClientError, ClientId, ClientSecret, HttpRequest, HttpResponse, RedirectUrl, Scope};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata},
    IssuerUrl,
};
use reqwest::Client as HttpClient;
use serde::Serialize;
use std::{num::TryFromIntError, time::Duration as StdDuration};
use std::{sync::Arc, time::Instant};
use thiserror::Error as ThisError;
use tokio::sync::Mutex;
use url::Url;

struct ClientInfo {
    client_id: ClientId,
    client_secret: ClientSecret,
    discovery_url: IssuerUrl,
    redirect_url: RedirectUrl,
    ttl_client: StdDuration,
}

#[derive(ThisError, Debug, Serialize)]
#[error("OpenId Connect discovery failed")]
#[serde(rename_all = "camelCase")]
pub struct OIDCDiscoveryError(pub String);

#[derive(Clone)]
struct CachedClient {
    client: CoreClient,
    created_at: Instant,
}

pub(in crate::auth) struct OIDCClient {
    pub provider: String,
    pub scopes: Vec<Scope>,
    client_info: ClientInfo,
    http_client: HttpClient,
    cached_client: Arc<Mutex<Option<CachedClient>>>,
}

impl OIDCClient {
    pub async fn new(
        provider: &str,
        auth_base_url: &Url,
        ignore_discovery_error: bool,
        config: &OIDCConfig,
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

        let ttl_client = config
            .ttl_client
            .map(|sec| Ok::<_, TryFromIntError>(StdDuration::from_secs(u64::try_from(sec)?)))
            .transpose()
            .map_err(AuthBuildError::InvalidKeyCacheTime)?
            .unwrap_or(StdDuration::from_secs(15 * 60));

        let ignore_certificates = config.ignore_certificates.unwrap_or(false);
        let http_client = HttpClient::builder()
            .redirect(reqwest::redirect::Policy::none())
            .danger_accept_invalid_certs(ignore_certificates)
            .build()
            .map_err(AuthBuildError::HttpClient)?;

        let client = Self {
            provider: provider.to_string(),
            scopes: config.scopes.iter().map(|scope| Scope::new(scope.clone())).collect(),
            client_info: ClientInfo {
                client_id,
                client_secret,
                discovery_url,
                redirect_url,
                ttl_client,
            },
            http_client,
            cached_client: Arc::new(Mutex::new(None)),
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

    pub async fn client(&self) -> Result<CoreClient, OIDCDiscoveryError> {
        let client_info = &self.client_info;

        // happy path, try to get the current client
        {
            let cached_client = self.cached_client.lock().await;
            if let Some(cached_client) = &*cached_client {
                let age = cached_client.created_at.elapsed();
                if age < self.client_info.ttl_client {
                    return Ok(cached_client.client.clone());
                }
                log::warn!("Discovery expired({}s) for {} ", self.provider, age.as_secs());
            }
        }

        // get client configuration from discovery
        let client = {
            let provider_metadata =
                match CoreProviderMetadata::discover_async(client_info.discovery_url.clone(), |request| async {
                    async_http_client(&self.http_client, request).await
                })
                .await
                {
                    Ok(meta) => meta,
                    Err(err) => {
                        log::warn!("Discovery failed for {}: {:#?}", self.provider, err);
                        return Err(OIDCDiscoveryError(format!("{err:#?}")));
                    }
                };

            CoreClient::from_provider_metadata(
                provider_metadata,
                client_info.client_id.clone(),
                Some(client_info.client_secret.clone()),
            )
            .set_redirect_uri(client_info.redirect_url.clone())
        };

        // cache the new client (last writer wins)
        {
            let mut cached_client = self.cached_client.lock().await;
            *cached_client = Some(CachedClient {
                created_at: Instant::now(),
                client: client.clone(),
            });
        }

        Ok(client)
    }

    pub async fn send_request(&self, request: HttpRequest) -> Result<HttpResponse, AsyncHttpClientError> {
        async_http_client(&self.http_client, request).await
    }
}
