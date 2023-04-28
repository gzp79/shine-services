use crate::{
    app_config::AppConfig,
    app_error::AppError,
    db::{self},
};
use sqlx::AnyPool;

#[derive(Clone)]
pub struct AppData {
    pool: AnyPool,
}

impl AppData {
    pub async fn new(config: &AppConfig) -> Result<Self, AppError> {
        let pool = db::create_pool(&config.db.connection_string).await?;

        Ok(Self { pool })
    }
}
