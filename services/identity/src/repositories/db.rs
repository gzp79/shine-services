use serde::{Deserialize, Serialize};
use shine_infra::db::{self, DBError, PGConnectionPool, RedisConnectionPool};

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
        log::debug!(
            "migrations: {:#?}",
            embedded::migrations::runner().get_migrations()
        );
        let client = &mut **backend;
        embedded::migrations::runner().run_async(client).await?;
        Ok(())
    }
}
