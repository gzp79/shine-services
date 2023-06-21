use crate::db::{DBConfig, DBError};
use shine_service::service::{self, PGConnectionPool, RedisConnectionPool};

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
        let postgres = service::create_postgres_pool(config.sql_cns.as_str())
            .await
            .map_err(DBError::PostgresPoolError)?;

        let redis = service::create_redis_pool(config.redis_cns.as_str())
            .await
            .map_err(DBError::RedisPoolError)?;

        let pool = Self { postgres, redis };
        pool.migrate().await?;
        Ok(pool)
    }

    async fn migrate(&self) -> Result<(), DBError> {
        let mut backend = self.postgres.get().await.map_err(DBError::PostgresPoolError)?;
        log::info!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
        embedded::migrations::runner().run_async(&mut *backend).await?;
        Ok(())
    }
}
