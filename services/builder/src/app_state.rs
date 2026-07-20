use crate::{
    app_config::AppConfig,
    repositories::DBPool,
    services::{HubService, SessionChecker},
    settings::{BuilderSettings, WsSettings},
};
use anyhow::{anyhow, Error as AnyError};
use regex::bytes::Regex;
use shine_infra::web::{CoreServices, WebAppConfig};
use std::{sync::Arc, time::Duration};

struct Inner {
    db: DBPool,
    hub_service: HubService,
    settings: BuilderSettings,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>, core_services: &CoreServices) -> Result<Self, AnyError> {
        let config_db = &config.feature.db;
        let config_ws = &config.feature.ws;

        let settings = {
            let allowed_origins = config
                .service
                .allowed_origins
                .iter()
                .map(|r| Regex::new(r))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| anyhow!("WebSocket origin config error: {err}"))?;
            let allowed_hosts = config_ws
                .allowed_hosts
                .iter()
                .map(|r| Regex::new(r))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| anyhow!("WebSocket host config error: {err}"))?;

            BuilderSettings {
                ws: WsSettings { allowed_origins, allowed_hosts },
            }
        };

        let db_pool = DBPool::new(config_db).await?;
        let hub_service = HubService::new();

        let auth_check_interval = Duration::from_secs(config.feature.auth_check_interval.max(1));
        SessionChecker::new(
            core_services.current_user_service.clone(),
            &hub_service,
            auth_check_interval,
        )
        .spawn()
        .await;

        Ok(Self(Arc::new(Inner {
            db: db_pool,
            hub_service,
            settings,
        })))
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }

    pub fn hub_service(&self) -> &HubService {
        &self.0.hub_service
    }

    pub fn settings(&self) -> &BuilderSettings {
        &self.0.settings
    }
}
