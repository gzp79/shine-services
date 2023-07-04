use crate::{
    auth::{
        ep_get_providers, ep_logout, ep_user_info, oidc_client::OIDCClient, oidc_ep_auth, oidc_ep_login,
        ExternalLoginMeta,
    },
    db::SettingsManager,
};
use axum::{response::Html, routing::get, Extension, Router};
use serde::{Deserialize, Serialize};
use shine_service::{axum::session::SessionError, service::DOMAIN_NAME};
use std::{collections::HashMap, sync::Arc};
use tera::Tera;
use thiserror::Error as ThisError;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCEndpoints {
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCConfig {
    pub discovery_url: Option<String>,
    pub endpoints: Option<OIDCEndpoints>,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub redirect_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    pub openid: HashMap<String, OIDCConfig>,
}

#[derive(Debug, ThisError)]
pub enum AuthBuildError {
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
    #[error("Missing OpenId discover or endpoint configuration")]
    InvalidEndpoints,
    #[error("Invalid redirect url: {0}")]
    RedirectUrl(String),
    #[error("Failed to discover open id: {0}")]
    Discovery(String),
}

pub struct AuthServiceBuilder {
    external_login_cookie_builder: ExternalLoginMeta,
    openid_clients: Vec<OIDCClient>,
}

impl AuthServiceBuilder {
    pub async fn new(config: &AuthConfig, cookie_secret: &str) -> Result<Self, AuthBuildError> {
        let mut openid_clients = Vec::new();
        for (provider, provider_config) in &config.openid {
            let connect = OIDCClient::new(provider, provider_config).await?;
            openid_clients.push(connect);
        }

        let external_login_cookie_builder = ExternalLoginMeta::new(cookie_secret)?
            .with_cookie_name("exl")
            .with_domain(DOMAIN_NAME);

        Ok(Self {
            external_login_cookie_builder,
            openid_clients,
        })
    }

    pub fn into_router<S>(self) -> (Router<S>, Router<S>)
    where
        S: Clone + Send + Sync + 'static,
    {
        let router = {
            let mut router = Router::new().route("/logout", get(ep_logout::logout));

            for openid_client in self.openid_clients {
                let path = format!("/{}", openid_client.provider);

                let openid_route = Router::new()
                    .route("/login", get(oidc_ep_login::openid_connect_login))
                    .route("/link", get(oidc_ep_login::openid_connect_link))
                    .route("/auth", get(oidc_ep_auth::openid_connect_auth))
                    .layer(Extension(Arc::new(openid_client)));

                router = router.nest(&path, openid_route);
            }

            router.layer(self.external_login_cookie_builder.into_layer())
        };

        let api_router = Router::new()
            .route("/userinfo", get(ep_user_info::user_info))
            .route("/providers", get(ep_get_providers::get_providers));

        (router, api_router)
    }
}

pub(in crate::auth) fn create_redirect_page(
    tera: &Tera,
    settings_manager: &SettingsManager,
    title: &str,
    target: &str,
    target_url: Option<&str>,
) -> Html<String> {
    let mut context = tera::Context::new();
    context.insert("title", title);
    context.insert("target", target);
    context.insert("redirect_url", target_url.unwrap_or(settings_manager.home_url()));
    let html = tera
        .render("redirect.html", &context)
        .expect("Failed to generate redirect.html template");
    Html(html)
}

pub(in crate::auth) fn create_ooops_page(
    tera: &Tera,
    settings_manager: &SettingsManager,
    detail: Option<&str>,
) -> Html<String> {
    let mut context = tera::Context::new();
    context.insert("home_url", settings_manager.home_url());
    context.insert("detail", &detail.unwrap_or_default());
    let html = tera
        .render("ooops.html", &context)
        .expect("Failed to generate ooops.html template");
    Html(html)
}
