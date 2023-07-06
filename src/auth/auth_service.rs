use crate::{
    auth::{
        ep_get_providers, ep_logout, ep_user_info, oauth2_client::OAuth2Client, oauth2_ep_auth, oauth2_ep_login,
        oidc_client::OIDCClient, oidc_ep_auth, oidc_ep_login, ExternalLoginMeta,
    },
    db::{IdentityManager, NameGenerator, SessionManager, SettingsManager},
};
use axum::{response::Html, routing::get, Extension, Router};
use serde::{Deserialize, Serialize};
use shine_service::{axum::session::SessionError, service::DOMAIN_NAME};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tera::Tera;
use thiserror::Error as ThisError;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCEndpoints {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Config {
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub user_info_mapping: HashMap<String, String>,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub redirect_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCConfig {
    pub discovery_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub redirect_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct AuthConfig {
    pub openid: HashMap<String, OIDCConfig>,
    pub oauth2: HashMap<String, OAuth2Config>,
}

#[derive(Debug, ThisError)]
pub enum AuthBuildError {
    #[error("Provider ({0}) already registered")]
    ProviderConflict(String),
    #[error(transparent)]
    InvalidSessionMeta(#[from] SessionError),
    #[error("Invalid issuer url: {0}")]
    InvalidIssuer(String),
    #[error("Invalid auth url: {0}")]
    InvalidAuth(String),
    #[error("Invalid token url: {0}")]
    InvalidToken(String),
    #[error("Invalid user info url: {0}")]
    InvalidUserInfo(String),
    #[error("Invalid redirect url: {0}")]
    RedirectUrl(String),
    #[error("Failed to discover open id: {0}")]
    Discovery(String),
}

#[derive(Clone)]
pub struct AuthServiceState {
    pub tera: Arc<Tera>,
    pub settings_manager: SettingsManager,
    pub identity_manager: IdentityManager,
    pub session_manager: SessionManager,
    pub name_generator: NameGenerator,
}

pub struct AuthServiceBuilder {
    state: AuthServiceState,
    external_login_cookie_builder: ExternalLoginMeta,
    openid_clients: Vec<OIDCClient>,
    oauth2_clients: Vec<OAuth2Client>,
}

impl AuthServiceBuilder {
    pub async fn new(
        state: AuthServiceState,
        config: &AuthConfig,
        cookie_secret: &str,
    ) -> Result<Self, AuthBuildError> {
        let mut providers = HashSet::new();

        let mut openid_clients = Vec::new();
        for (provider, provider_config) in &config.openid {
            if !providers.insert(provider.clone()) {
                return Err(AuthBuildError::ProviderConflict(provider.clone()));
            }

            let connect = OIDCClient::new(provider, provider_config).await?;
            openid_clients.push(connect);
        }

        let mut oauth2_clients = Vec::new();
        for (provider, provider_config) in &config.oauth2 {
            if !providers.insert(provider.clone()) {
                return Err(AuthBuildError::ProviderConflict(provider.clone()));
            }

            let connect = OAuth2Client::new(provider, provider_config).await?;
            oauth2_clients.push(connect);
        }

        let external_login_cookie_builder = ExternalLoginMeta::new(cookie_secret)?
            .with_cookie_name("exl")
            .with_domain(DOMAIN_NAME);

        Ok(Self {
            state,
            external_login_cookie_builder,
            openid_clients,
            oauth2_clients,
        })
    }

    pub fn into_router<S>(self) -> (Router<S>, Router<S>)
    where
        S: Clone + Send + Sync + 'static,
    {
        let router = {
            let mut router = Router::new().route("/auth/logout", get(ep_logout::logout));

            for openid_client in self.openid_clients {
                let path = format!("/auth/{}", openid_client.provider);

                let openid_route = Router::new()
                    .route("/login", get(oidc_ep_login::openid_connect_login))
                    .route("/link", get(oidc_ep_login::openid_connect_link))
                    .route("/auth", get(oidc_ep_auth::openid_connect_auth))
                    .layer(Extension(Arc::new(openid_client)));

                router = router.nest(&path, openid_route);
            }

            for oauth2_client in self.oauth2_clients {
                let path = format!("/auth/{}", oauth2_client.provider);

                let openid_route = Router::new()
                    .route("/login", get(oauth2_ep_login::oauth2_connect_login))
                    .route("/link", get(oauth2_ep_login::oauth2_connect_link))
                    .route("/auth", get(oauth2_ep_auth::oauth2_connect_auth))
                    .layer(Extension(Arc::new(oauth2_client)));

                router = router.nest(&path, openid_route);
            }

            router
                .layer(self.external_login_cookie_builder.into_layer())
                .with_state(self.state.clone())
        };

        let api_router = Router::new()
            .route("/auth/userinfo", get(ep_user_info::user_info))
            .route("/auth/providers", get(ep_get_providers::get_providers))
            .with_state(self.state);

        (router, api_router)
    }
}

pub(in crate::auth) fn create_redirect_page(
    state: &AuthServiceState,
    title: &str,
    target: &str,
    target_url: Option<&str>,
) -> Html<String> {
    let mut context = tera::Context::new();
    context.insert("title", title);
    context.insert("target", target);
    context.insert("redirect_url", target_url.unwrap_or(state.settings_manager.home_url()));
    let html = state
        .tera
        .render("redirect.html", &context)
        .expect("Failed to generate redirect.html template");
    Html(html)
}

pub(in crate::auth) fn create_ooops_page(state: &AuthServiceState, detail: Option<&str>) -> Html<String> {
    let mut context = tera::Context::new();
    context.insert("home_url", state.settings_manager.home_url());
    context.insert("detail", &detail.unwrap_or_default());
    let html = state
        .tera
        .render("ooops.html", &context)
        .expect("Failed to generate ooops.html template");
    Html(html)
}
