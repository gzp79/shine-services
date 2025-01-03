use serde::{Deserialize, Serialize};
use shine_core::db::{
    self, PGConnectionError, PGConnectionPool, PGCreatePoolError, PGError, RedisConnectionError, RedisConnectionPool,
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum DBError {
    #[error("Failed to get a PG connection from the pool")]
    PGCreatePoolError(#[source] PGCreatePoolError),
    #[error("Failed to get a PG connection from the pool")]
    PGPoolError(#[source] PGConnectionError),
    #[error(transparent)]
    PGError(#[from] PGError),
    #[error(transparent)]
    SqlMigration(#[from] refinery::Error),

    #[error("Failed to get pooled redis connection")]
    RedisPoolError(#[source] RedisConnectionError),
    #[error(transparent)]
    RedisError(#[from] redis::RedisError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub sql_cns: String,
    pub redis_cns: String,
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

#[derive(Clone)]
pub struct DBPool {
    pub postgres: PGConnectionPool,
    pub redis: RedisConnectionPool,
}

impl DBPool {
    pub async fn new(config: &DBConfig) -> Result<Self, DBError> {
        let postgres = db::create_postgres_pool(config.sql_cns.as_str())
            .await
            .map_err(DBError::PGCreatePoolError)?;

        let redis = db::create_redis_pool(config.redis_cns.as_str())
            .await
            .map_err(DBError::RedisPoolError)?;

        let pool = Self { postgres, redis };
        pool.migrate().await?;
        Ok(pool)
    }

    async fn migrate(&self) -> Result<(), DBError> {
        let mut backend = self.postgres.get().await.map_err(DBError::PGPoolError)?;
        log::info!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
        let client = &mut **backend;
        embedded::migrations::runner().run_async(client).await?;
        Ok(())
    }
}
