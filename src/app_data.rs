use crate::{
    app_config::AppConfig,
    app_error::AppError,
    db::{self, IdentityManager},
};

#[derive(Clone)]
pub struct AppData {
    pub identity_manager: IdentityManager,
}

impl AppData {
    pub async fn new(config: &AppConfig) -> Result<Self, AppError> {
        let pool = db::create_pool(&config.db.connection_string).await?;
        let identity_manager = IdentityManager::new(pool);

        Ok(Self { identity_manager })
    }
}
