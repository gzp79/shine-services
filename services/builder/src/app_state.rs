use crate::{app_config::AppConfig, repositories::DBPool, services::SessionHandler};
use anyhow::{anyhow, Error as AnyError};
use regex::bytes::Regex;
use ring::rand::SystemRandom;
use shine_infra::web::WebAppConfig;
use std::sync::Arc;

struct Inner {
    random: SystemRandom,
    db: DBPool,
    sessions: SessionHandler,
    ws_allowed_origins: Vec<Regex>,
    ws_allowed_hosts: Vec<Regex>,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        let config_db = &config.feature.db;
        let ws_allowed_origins = config
            .service
            .allowed_origins
            .iter()
            .map(|r| Regex::new(r))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| anyhow!("WebSocket origin config error: {err}"))?;
        let ws_allowed_hosts = config
            .service
            .allowed_ws_hosts
            .iter()
            .map(|r| Regex::new(r))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| anyhow!("WebSocket host config error: {err}"))?;

        let db_pool = DBPool::new(config_db).await?;

        Ok(Self(Arc::new(Inner {
            random: SystemRandom::new(),
            db: db_pool,
            sessions: SessionHandler::new(),
            ws_allowed_origins,
            ws_allowed_hosts,
        })))
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }

    pub fn sessions(&self) -> &SessionHandler {
        &self.0.sessions
    }

    pub fn is_allowed_ws_origin(&self, origin: &[u8]) -> bool {
        self.0.ws_allowed_origins.iter().any(|r| r.is_match(origin))
    }

    pub fn is_allowed_ws_host(&self, host: &[u8]) -> bool {
        self.0.ws_allowed_hosts.iter().any(|r| r.is_match(host))
    }
}
