pub use self::auth_error::*;
mod auth_session;
pub use self::auth_session::*;
mod auth_page;
pub use self::auth_page::*;
mod page_utils;
pub use self::page_utils::*;
mod captcha_utils;
pub use self::captcha_utils::*;
mod link_utils;
pub use self::link_utils::*;
mod oauth2_client;
pub use self::oauth2_client::*;
mod oidc_client;
pub use self::oidc_client::*;

mod api;
mod pages;

use crate::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use axum::Extension;
use shine_core::web::WebAppConfig;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
mod auth_error;
use std::sync::Arc;

pub struct AuthController {
    auth_session_meta: AuthSessionMeta,
    oauth2_clients: Vec<OAuth2Client>,
    openid_clients: Vec<OIDCClient>,
}

impl AuthController {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        let config_auth = &config.feature.auth;

        let auth_session_meta = AuthSessionMeta::new(config)?;

        let mut oauth2_clients = Vec::new();
        for (provider, provider_config) in &config_auth.oauth2 {
            let connect = OAuth2Client::new(provider, &config_auth.auth_base_url, provider_config).await?;
            oauth2_clients.push(connect);
        }

        let openid_startup_discovery = config_auth.openid_startup_discovery;
        let mut openid_clients = Vec::new();
        for (provider, provider_config) in &config_auth.openid {
            if let Some(connect) = OIDCClient::new(
                provider,
                &config_auth.auth_base_url,
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

        Ok(Self {
            auth_session_meta,
            oauth2_clients,
            openid_clients,
        })
    }

    pub fn into_router(self) -> OpenApiRouter<AppState> {
        let mut auth_routes = OpenApiRouter::new()
            .routes(routes!(pages::token_login))
            .routes(routes!(pages::validate))
            .routes(routes!(pages::logout))
            .routes(routes!(pages::delete_user));

        for client in self.oauth2_clients {
            log::info!("Registering OAuth2 provider {}", client.provider);

            let provider_route = OpenApiRouter::new()
                .nest(
                    &format!("/auth/{}", client.provider),
                    OpenApiRouter::new().routes(routes!(pages::oauth2_login)),
                )
                .nest(
                    &format!("/auth/{}", client.provider),
                    OpenApiRouter::new().routes(routes!(pages::oauth2_link)),
                )
                .nest(
                    &format!("/auth/{}", client.provider),
                    OpenApiRouter::new().routes(routes!(pages::oauth2_auth)),
                )
                .layer(Extension(Arc::new(client)));

            auth_routes = auth_routes.merge(provider_route);
        }

        for client in self.openid_clients {
            log::info!("Registering OpenId Connect provider {}", client.provider);

            let provider_route = OpenApiRouter::new()
                .nest(
                    &format!("/auth/{}", client.provider),
                    OpenApiRouter::new().routes(routes!(pages::oidc_login)),
                )
                .nest(
                    &format!("/auth/{}", client.provider),
                    OpenApiRouter::new().routes(routes!(pages::oidc_link)),
                )
                .nest(
                    &format!("/auth/{}", client.provider),
                    OpenApiRouter::new().routes(routes!(pages::oidc_auth)),
                )
                .layer(Extension(Arc::new(client)));

            auth_routes = auth_routes.merge(provider_route);
        }

        auth_routes = auth_routes.layer(self.auth_session_meta.into_layer());

        let api_routes = OpenApiRouter::new()
            .routes(routes!(api::get_user_info))
            .routes(routes!(api::create_token))
            .routes(routes!(api::get_token))
            .routes(routes!(api::list_tokens))
            .routes(routes!(api::delete_token))
            .routes(routes!(api::list_external_providers))
            .routes(routes!(api::list_external_links))
            .routes(routes!(api::delete_external_link))
            .routes(routes!(api::list_sessions));

        auth_routes.merge(api_routes)
    }
}
