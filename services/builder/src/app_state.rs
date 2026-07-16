use crate::{
    app_config::AppConfig,
    repositories::DBPool,
    services::SessionHandler,
    settings::{BuilderSettings, WsSettings},
};
use anyhow::{anyhow, Error as AnyError};
use regex::bytes::Regex;
use ring::rand::SystemRandom;
use shine_infra::web::WebAppConfig;
use std::{sync::Arc, time::Duration};

struct Inner {
    random: SystemRandom,
    db: DBPool,
    sessions: SessionHandler,
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
            let auth_check_interval =
                Duration::from_secs(u64::try_from(config_ws.auth_check_interval).unwrap_or(1).max(1));

            BuilderSettings {
                ws: WsSettings {
                    allowed_origins,
                    allowed_hosts,
                    auth_check_interval,
                },
            }
        };

        let db_pool = DBPool::new(config_db).await?;

        Ok(Self(Arc::new(Inner {
            random: SystemRandom::new(),
            db: db_pool,
            sessions: SessionHandler::new(),
            settings,
        })))
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }

    pub fn sessions(&self) -> &SessionHandler {
        &self.0.sessions
    }

    pub fn settings(&self) -> &BuilderSettings {
        &self.0.settings
    }
}
