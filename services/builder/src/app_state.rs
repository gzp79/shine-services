use crate::{app_config::AppConfig, repositories::DBPool};
use anyhow::Error as AnyError;
use ring::rand::SystemRandom;
use shine_core::web::WebAppConfig;
use std::sync::Arc;

struct Inner {
    random: SystemRandom,
    db: DBPool,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    pub async fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AnyError> {
        let config_db = &config.feature.builder_db;

        let db_pool = DBPool::new(config_db).await?;

        Ok(Self(Arc::new(Inner {
            random: SystemRandom::new(),
            db: db_pool,
        })))
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }
}
