use crate::{
    auth::{self, AuthSessionMeta, OAuth2Client, OIDCClient, OIDCDiscoveryError},
    repositories::{AutoNameManager, IdentityManager, SessionManager},
};
use axum::{Extension, Router};
use chrono::Duration;
use oauth2::{reqwest::AsyncHttpClientError, HttpRequest, HttpResponse};
use reqwest::Client as HttpClient;
use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use shine_service::axum::ApiRoute;
use std::{
    collections::{HashMap, HashSet},
    num::TryFromIntError,
    sync::Arc,
};
use tera::Tera;
use thiserror::Error as ThisError;
use url::Url;
use utoipa::openapi::OpenApi;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExternalUserInfoExtensions {
    GithubEmail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Config {
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub user_info_mapping: HashMap<String, String>,
    pub extensions: HashSet<ExternalUserInfoExtensions>,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub ignore_certificates: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCConfig {
    pub discovery_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub ignore_certificates: Option<bool>,
    /// Maximum time to store the discovered OIDC client information, like JWKS.
    pub ttl_client: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionConfig {
    pub cookie_name_suffix: Option<String>,

    pub session_secret: String,
    pub external_login_cookie_secret: String,
    pub token_cookie_secret: String,

    /// The maximum time to line of a session in seconds
    pub ttl_session: usize,
    /// The maximum time to line of an access (remember me) token in seconds
    pub ttl_access_token: usize,
    /// The maximum time to line of a single access (one-time-use) token in seconds
    pub ttl_single_access: usize,
    /// The maximum time to line of an api-key in seconds
    pub ttl_api_key: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    /// The name of the application
    pub app_name: String,
    /// The default redirection URL for users
    pub home_url: Url,
    /// The default redirection URL for users in case of an err
    pub error_url: Url,
    /// The URL base for authentication services. This serves as:
    /// - The source for certain cookie protection parameters, including domain and path.
    /// - The base URL for managing the redirection of external login authentication flows.
    pub auth_base_url: Url,

    /// Hash of the master key to manage user roles. In production once a user is created it's highly
    /// recommended to disable this feature.
    pub super_user_api_key_hash: Option<String>,

    /// Authentication related cookie configuration.
    #[serde(flatten)]
    pub auth_session: AuthSessionConfig,

    /// The time interval before redirecting the user from embedded pages. If not provided, no redirection occurs,
    /// and a value of 0 signifies an immediate redirect.
    page_redirect_time: Option<u32>,
    /// Enable to display error details on pages. From a security standpoint, it is not advisable to enable this feature
    /// as it may expose unwanted information. Therefore, it is recommended to disable this feature in production.
    page_error_detail: Option<bool>,

    /// Initiates OIDC discovery at startup to identify and rectify any potential misconfigurations.
    openid_startup_discovery: bool,
    /// A list of external providers utilizing the (interactive) OpenID Connect login flow.
    pub openid: HashMap<String, OIDCConfig>,
    /// List of external providers utilizing the (interactive) OAuth2 login flow
    pub oauth2: HashMap<String, OAuth2Config>,
}

#[derive(Debug, ThisError)]
pub enum AuthBuildError {
    #[error("Invalid token duration")]
    InvalidTokenDuration(#[from] TryFromIntError),
    #[error("Provider ({0}) already registered")]
    ProviderConflict(String),
    #[error("Auth session error: {0}")]
    InvalidAuthSession(String),
    #[error("Invalid issuer url: {0}")]
    InvalidIssuer(String),
    #[error("Invalid auth url: {0}")]
    InvalidAuthUrl(String),
    #[error("Invalid token url: {0}")]
    InvalidTokenUrl(String),
    #[error("Invalid user info url: {0}")]
    InvalidUserInfoUrl(String),
    #[error("Invalid redirect url: {0}")]
    RedirectUrl(String),
    #[error("Invalid key cache time: {0}")]
    InvalidKeyCacheTime(#[source] TryFromIntError),
    #[error("Failed to discover open id: {0}")]
    OIDCDiscovery(OIDCDiscoveryError),
    #[error("Failed to create http client")]
    HttpClient(#[source] reqwest::Error),
}

struct Inner {
    tera: Tera,
    identity_manager: IdentityManager,
    session_manager: SessionManager,
    auto_name_manager: AutoNameManager,
    random: SystemRandom,

    app_name: String,
    home_url: Url,
    error_url: Url,

    ttl_access_token: Duration,
    ttl_single_access: Duration,
    ttl_api_key: Duration,

    page_redirect_time: i64,
    page_error_detail: bool,
    providers: Vec<String>,
}

#[derive(Clone)]
pub struct AuthServiceState(Arc<Inner>);

impl AuthServiceState {
    pub fn tera(&self) -> &Tera {
        &self.0.tera
    }

    pub fn identity_manager(&self) -> &IdentityManager {
        &self.0.identity_manager
    }

    pub fn session_manager(&self) -> &SessionManager {
        &self.0.session_manager
    }

    pub fn auto_name_manager(&self) -> &AutoNameManager {
        &self.0.auto_name_manager
    }

    pub fn random(&self) -> &SystemRandom {
        &self.0.random
    }

    pub fn app_name(&self) -> &str {
        &self.0.app_name
    }

    pub fn home_url(&self) -> &Url {
        &self.0.home_url
    }

    pub fn error_url(&self) -> &Url {
        &self.0.error_url
    }

    pub fn ttl_access_token(&self) -> &Duration {
        &self.0.ttl_access_token
    }

    pub fn ttl_single_access(&self) -> &Duration {
        &self.0.ttl_single_access
    }

    pub fn ttl_api_key(&self) -> &Duration {
        &self.0.ttl_api_key
    }

    pub fn page_redirect_time(&self) -> i64 {
        self.0.page_redirect_time
    }

    pub fn page_error_detail(&self) -> bool {
        self.0.page_error_detail
    }

    pub fn providers(&self) -> &[String] {
        &self.0.providers
    }
}

pub struct AuthServiceDependencies {
    pub tera: Tera,
    pub identity_manager: IdentityManager,
    pub session_manager: SessionManager,
    pub auto_name_manager: AutoNameManager,
}

pub struct AuthServiceBuilder {
    state: AuthServiceState,
    auth_session_meta: AuthSessionMeta,
    openid_clients: Vec<OIDCClient>,
    oauth2_clients: Vec<OAuth2Client>,
}

impl AuthServiceBuilder {
    pub async fn new(dependencies: AuthServiceDependencies, config: &AuthConfig) -> Result<Self, AuthBuildError> {
        let mut providers = HashSet::new();

        let ttl_access_token = Duration::seconds(i64::try_from(config.auth_session.ttl_access_token)?);
        let ttl_single_access = Duration::seconds(i64::try_from(config.auth_session.ttl_single_access)?);
        let ttl_api_key = Duration::seconds(i64::try_from(config.auth_session.ttl_api_key)?);

        let openid_startup_discovery = config.openid_startup_discovery;
        let mut openid_clients = Vec::new();
        for (provider, provider_config) in &config.openid {
            if !providers.insert(provider.clone()) {
                return Err(AuthBuildError::ProviderConflict(provider.clone()));
            }

            if let Some(connect) = OIDCClient::new(
                provider,
                &config.auth_base_url,
                openid_startup_discovery,
                provider_config,
            )
            .await?
            {
                openid_clients.push(connect);
            } else {
                log::error!("Skipping {provider} provider");
            }
        }

        let mut oauth2_clients = Vec::new();
        for (provider, provider_config) in &config.oauth2 {
            if !providers.insert(provider.clone()) {
                return Err(AuthBuildError::ProviderConflict(provider.clone()));
            }

            let connect = OAuth2Client::new(provider, &config.auth_base_url, provider_config).await?;
            oauth2_clients.push(connect);
        }

        let state = AuthServiceState(Arc::new(Inner {
            tera: dependencies.tera,
            identity_manager: dependencies.identity_manager,
            session_manager: dependencies.session_manager,
            auto_name_manager: dependencies.auto_name_manager,
            random: SystemRandom::new(),
            app_name: config.app_name.to_owned(),
            home_url: config.home_url.to_owned(),
            error_url: config.error_url.to_owned(),
            ttl_access_token,
            ttl_single_access,
            ttl_api_key,
            page_redirect_time: config.page_redirect_time.map(i64::from).unwrap_or(-1),
            page_error_detail: config.page_error_detail.unwrap_or(false),
            providers: providers.into_iter().collect(),
        }));

        let auth_session_meta = AuthSessionMeta::new(
            config.home_url.clone(),
            config.auth_base_url.clone(),
            &config.auth_session,
        )
        .map_err(|err| AuthBuildError::InvalidAuthSession(format!("{err}")))?;

        Ok(Self {
            state,
            auth_session_meta,
            openid_clients,
            oauth2_clients,
        })
    }

    pub fn into_router<S>(self, doc: &mut OpenApi) -> (Router<S>, Router<S>)
    where
        S: Clone + Send + Sync + 'static,
    {
        let page_router = {
            let mut router = Router::new()
                .add_api(auth::page_logout(), doc)
                .add_api(auth::page_validate(), doc)
                .add_api(auth::page_delete_user(), doc);

            router = router.nest("", Router::new().add_api(auth::page_token_login(), doc));

            for client in self.openid_clients {
                log::info!("Registering OpenId Connect provider {}", client.provider);

                router = router.nest(
                    "",
                    Router::new()
                        .add_api(auth::page_oidc_login(&client.provider), doc)
                        .add_api(auth::page_oidc_link(&client.provider), doc)
                        .add_api(auth::page_oidc_auth(&client.provider), doc)
                        .layer(Extension(Arc::new(client))),
                );
            }

            for client in self.oauth2_clients {
                log::info!("Registering OAuth2 provider {}", client.provider);

                router = router.nest(
                    "",
                    Router::new()
                        .add_api(auth::page_oauth2_login(&client.provider), doc)
                        .add_api(auth::page_oauth2_link(&client.provider), doc)
                        .add_api(auth::page_oauth2_auth(&client.provider), doc)
                        .layer(Extension(Arc::new(client))),
                );
            }

            router
                .layer(self.auth_session_meta.into_layer())
                .with_state(self.state.clone())
        };

        let api_router = Router::new()
            .add_api(auth::ep_provider_list(), doc)
            .add_api(auth::ep_user_info_get(), doc)
            .add_api(auth::ep_session_list(), doc)
            .add_api(auth::ep_token_create(), doc)
            .add_api(auth::ep_token_get(), doc)
            .add_api(auth::ep_token_list(), doc)
            .add_api(auth::ep_token_delete(), doc)
            .add_api(auth::ep_external_link_list(), doc)
            .add_api(auth::ep_external_link_delete(), doc)
            .with_state(self.state);

        (page_router, api_router)
    }
}

pub async fn async_http_client(
    http_client: &HttpClient,
    request: HttpRequest,
) -> Result<HttpResponse, AsyncHttpClientError> {
    let mut request_builder = http_client
        .request(request.method, request.url.as_str())
        .body(request.body);
    for (name, value) in &request.headers {
        request_builder = request_builder.header(name.as_str(), value.as_bytes());
    }
    let request = request_builder.build().map_err(AsyncHttpClientError::Reqwest)?;

    let response = http_client
        .execute(request)
        .await
        .map_err(AsyncHttpClientError::Reqwest)?;

    let status_code = response.status();
    let headers = response.headers().to_owned();
    let chunks = response.bytes().await.map_err(AsyncHttpClientError::Reqwest)?;
    Ok(HttpResponse {
        status_code,
        headers,
        body: chunks.to_vec(),
    })
}

#[cfg(test)]
mod test {
    use axum_extra::extract::cookie::Key;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
    use shine_test::test;

    #[test]
    #[ignore = "This is not a test but a helper to generate secret"]
    fn generate_secret() {
        let key = Key::generate();
        println!("{}", B64.encode(key.master()));
    }
}
