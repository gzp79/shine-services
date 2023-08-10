use crate::{
    auth::{self, AuthSessionMeta, OAuth2Client, OIDCClient, TokenGenerator},
    db::{IdentityManager, NameGenerator, SessionManager},
};
use axum::{Extension, Router};
use chrono::Duration;
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCConfig {
    pub discovery_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionConfig {
    pub cookie_name_suffix: Option<String>,

    pub session_secret: String,
    pub external_login_secret: String,
    pub token_login_secret: String,

    pub ttl_session: usize,
    pub ttl_remember_me: usize,
    pub ttl_single_access: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    /// Default url where user is redirected to
    pub home_url: Url,
    /// Default url where user is redirected to in case of error
    pub error_url: Url,
    /// The url base for the authentication services:
    /// - source for some cookie protection parameters (domain, path)
    /// - redirect url base for external logins
    pub auth_base_url: Url,
    /// Time before redirection to user from the embedded pages. If not given, no redirect happens
    /// and a value of 0 implies an immediate redirect.
    page_redirect_time: Option<u32>,

    /// Auth related cookie parameters.
    #[serde(flatten)]
    pub auth_session: AuthSessionConfig,

    /// If enabled, when openid discovery fails, continue, but skip the provider. It is mainly used for testing where
    /// mocking of openid is not complete.
    openid_ignore_discovery_error: Option<bool>,
    /// List of external providers using the OpenId Connect protocol
    pub openid: HashMap<String, OIDCConfig>,
    /// List of external providers using OAuth2 protocols
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
    #[error("Failed to discover open id: {0}")]
    Discovery(String),
}

struct Inner {
    tera: Tera,
    identity_manager: IdentityManager,
    session_manager: SessionManager,
    name_generator: NameGenerator,

    home_url: Url,
    error_url: Url,
    page_redirect_time: i64,
    providers: Vec<String>,
    token_generator: TokenGenerator,
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

    pub fn name_generator(&self) -> &NameGenerator {
        &self.0.name_generator
    }

    pub fn token(&self) -> &TokenGenerator {
        &self.0.token_generator
    }

    pub fn home_url(&self) -> &Url {
        &self.0.home_url
    }

    pub fn error_url(&self) -> &Url {
        &self.0.error_url
    }

    pub fn page_redirect_time(&self) -> i64 {
        self.0.page_redirect_time
    }

    pub fn providers(&self) -> &[String] {
        &self.0.providers
    }
}

pub struct AuthServiceDependencies {
    pub tera: Tera,
    pub identity_manager: IdentityManager,
    pub session_manager: SessionManager,
    pub name_generator: NameGenerator,
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

        let ttl_remember_me = Duration::seconds(i64::try_from(config.auth_session.ttl_remember_me)?);
        let ttl_single_access = Duration::seconds(i64::try_from(config.auth_session.ttl_single_access)?);
        let token_generator = TokenGenerator::new(ttl_remember_me, ttl_single_access);

        let openid_ignore_discovery_error = config.openid_ignore_discovery_error.unwrap_or(false);
        let mut openid_clients = Vec::new();
        for (provider, provider_config) in &config.openid {
            if !providers.insert(provider.clone()) {
                return Err(AuthBuildError::ProviderConflict(provider.clone()));
            }

            if let Some(connect) = OIDCClient::new(
                provider,
                &config.auth_base_url,
                provider_config,
                openid_ignore_discovery_error,
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
            name_generator: dependencies.name_generator,
            token_generator,
            home_url: config.home_url.to_owned(),
            error_url: config.error_url.to_owned(),
            page_redirect_time: config.page_redirect_time.map(i64::from).unwrap_or(-1),
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
            .add_api(auth::ep_get_user_info(), doc)
            .add_api(auth::ep_get_auth_providers(), doc)
            .add_api(auth::ep_create_token(), doc)
            .with_state(self.state);

        (page_router, api_router)
    }
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
