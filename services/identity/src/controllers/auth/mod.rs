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

use super::AppState;
use crate::app_config::AppConfig;
use anyhow::Error as AnyError;
use axum::Extension;
use axum::Router;
use shine_service::axum::ApiRoute;
use utoipa::openapi::OpenApi;
mod auth_error;
use std::sync::Arc;

pub struct AuthController {
    auth_session_meta: AuthSessionMeta,
    oauth2_clients: Vec<OAuth2Client>,
    openid_clients: Vec<OIDCClient>,
}

impl AuthController {
    pub async fn new(config: &AppConfig) -> Result<Self, AnyError> {
        let auth_session_meta = AuthSessionMeta::new(
            config.auth.home_url.clone(),
            config.auth.auth_base_url.clone(),
            &config.auth.auth_session,
        )?;

        let mut oauth2_clients = Vec::new();
        for (provider, provider_config) in &config.auth.oauth2 {
            let connect = OAuth2Client::new(provider, &config.auth.auth_base_url, provider_config).await?;
            oauth2_clients.push(connect);
        }

        let openid_startup_discovery = config.auth.openid_startup_discovery;
        let mut openid_clients = Vec::new();
        for (provider, provider_config) in &config.auth.openid {
            if let Some(connect) = OIDCClient::new(
                provider,
                &config.auth.auth_base_url,
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

    pub fn into_router(self, doc: &mut OpenApi) -> Router<AppState> {
        let mut auth_routes = Router::new()
            .add_api(pages::page_token_login(), doc)
            .add_api(pages::page_validate(), doc)
            .add_api(pages::page_logout(), doc)
            .add_api(pages::page_delete_user(), doc);

        for client in self.oauth2_clients {
            log::info!("Registering OAuth2 provider {}", client.provider);

            auth_routes = auth_routes.nest(
                "",
                Router::new()
                    .add_api(pages::page_oauth2_login(&client.provider), doc)
                    .add_api(pages::page_oauth2_link(&client.provider), doc)
                    .add_api(pages::page_oauth2_auth(&client.provider), doc)
                    .layer(Extension(Arc::new(client))),
            );
        }

        for client in self.openid_clients {
            log::info!("Registering OpenId Connect provider {}", client.provider);

            auth_routes = auth_routes.nest(
                "",
                Router::new()
                    .add_api(pages::page_oidc_login(&client.provider), doc)
                    .add_api(pages::page_oidc_link(&client.provider), doc)
                    .add_api(pages::page_oidc_auth(&client.provider), doc)
                    .layer(Extension(Arc::new(client))),
            );
        }

        let auth_routes = auth_routes.layer(self.auth_session_meta.into_layer());

        let api_routes = Router::new()
            .add_api(api::ep_get_user_info(), doc)
            .add_api(api::ep_create_token(), doc)
            .add_api(api::ep_get_token(), doc)
            .add_api(api::ep_list_tokens(), doc)
            .add_api(api::ep_delete_token(), doc)
            .add_api(api::ep_list_external_providers(), doc)
            .add_api(api::ep_list_external_links(), doc)
            .add_api(api::ep_delete_external_link(), doc)
            .add_api(api::ep_list_sessions(), doc);

        Router::new().merge(auth_routes).merge(api_routes)
    }
}
