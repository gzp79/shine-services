use super::{api, pages, AuthSessionMeta, OAuth2Client, OIDCClient};
use crate::{app_config::AppConfig, app_state::AppState};
use anyhow::Error as AnyError;
use shine_infra::web::WebAppConfig;
use utoipa_axum::router::OpenApiRouter;

pub struct AuthRouter {
    auth_session_meta: AuthSessionMeta,
    oauth2_clients: Vec<OAuth2Client>,
    openid_clients: Vec<OIDCClient>,
}

impl AuthRouter {
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
        let mut auth_routes = pages::page_routes();

        for client in self.oauth2_clients {
            log::info!("Registering OAuth2 provider {}", client.provider);

            let provider_route = pages::oauth2_provider_routes(client);

            auth_routes = auth_routes.merge(provider_route);
        }

        for client in self.openid_clients {
            log::info!("Registering OpenId Connect provider {}", client.provider);

            let provider_route = pages::oidc_provider_routes(client);

            auth_routes = auth_routes.merge(provider_route);
        }

        auth_routes = auth_routes.layer(self.auth_session_meta.into_layer());

        let api_routes = api::api_routes();

        auth_routes.merge(api_routes)
    }
}
