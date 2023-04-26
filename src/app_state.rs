use crate::{app_config::AppConfig, app_error::AppError, db::Migrations};
use sqlx::{any::AnyPoolOptions, migrate::MigrateDatabase, AnyPool};

#[derive(Clone)]
pub struct AppState {
    pool: AnyPool,
    //identity_manager: IdentityManager,
}

impl AppState {
    async fn create_pool(cns: &str) -> Result<AnyPool, AppError> {
        if !sqlx::Any::database_exists(cns).await? {
            sqlx::Any::create_database(cns).await?;
        }

        let pool = AnyPoolOptions::new().max_connections(5).connect(cns).await?;
        Migrations.apply(&pool).await?;
        Ok(pool)
    }

    pub async fn new(config: &AppConfig) -> Result<Self, AppError> {
        let pool = Self::create_pool(&config.db.connection_string).await?;

        Ok(Self { pool })
    }
}
