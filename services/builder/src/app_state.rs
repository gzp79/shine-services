use crate::{
    app_config::AppConfig,
    repositories::DBPool,
    services::{HubService, SessionChecker},
    settings::{BuilderSettings, WsSettings},
};
use anyhow::{anyhow, Error as AnyError};
use regex::bytes::Regex;
use ring::rand::SystemRandom;
use shine_infra::{session::CurrentUserService, web::WebAppConfig};
use std::{sync::Arc, time::Duration};

struct Inner {
    random: SystemRandom,
    db: DBPool,
    hub_service: HubService,
    settings: BuilderSettings,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
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

        // Dedicated CurrentUserService instance for the session checker.
        // Duplicates one Redis pool versus the Extension-injected instance
        // used by request handlers (crates/shine-infra/src/web/web_app.rs) —
        // AppState::new runs before that layer is attached, so it can't reuse it.
        let session_service = Arc::new(
            CurrentUserService::from_config(&config.service)
                .await
                .map_err(|err| anyhow!("Failed to create session checker's CurrentUserService: {err}"))?,
        );
        let auth_check_interval =
            Duration::from_secs(u64::try_from(config.feature.auth_check_interval).unwrap_or(1).max(1));
        let _session_checker = SessionChecker::spawn(&hub_service, session_service, auth_check_interval).await;

        Ok(Self(Arc::new(Inner {
            random: SystemRandom::new(),
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
