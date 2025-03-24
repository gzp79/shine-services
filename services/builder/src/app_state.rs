use crate::{app_config::AppConfig, repositories::DBPool, services::SessionHandler};
use anyhow::Error as AnyError;
use ring::rand::SystemRandom;
use shine_infra::web::WebAppConfig;
use std::sync::Arc;

struct Inner {
    random: SystemRandom,
    db: DBPool,
    sessions: SessionHandler,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        let config_db = &config.feature.db;

        let db_pool = DBPool::new(config_db).await?;

        Ok(Self(Arc::new(Inner {
            random: SystemRandom::new(),
            db: db_pool,
            sessions: SessionHandler::new(),
        })))
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }

    pub fn sessions(&self) -> &SessionHandler {
        &self.0.sessions
    }
}
