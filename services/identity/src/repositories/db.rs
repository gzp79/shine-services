use serde::{Deserialize, Serialize};
use shine_infra::db::{DBError, PGConnectionPool, RedisConnectionPool};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailProtectionConfig {
    pub encryption_key: String,
    pub hash_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub sql_cns: String,
    pub redis_cns: String,
    pub email_protection: EmailProtectionConfig,
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

pub async fn create_postgres_pool(config: &DBConfig) -> Result<PGConnectionPool, DBError> {
    let postgres = shine_infra::db::create_postgres_pool(config.sql_cns.as_str())
        .await
        .map_err(DBError::PGCreatePoolError)?;

    migrate_postgres(&postgres).await?;
    Ok(postgres)
}

pub async fn create_redis_pool(config: &DBConfig) -> Result<RedisConnectionPool, DBError> {
    shine_infra::db::create_redis_pool(config.redis_cns.as_str())
        .await
        .map_err(DBError::RedisPoolError)
}

async fn migrate_postgres(postgres: &PGConnectionPool) -> Result<(), DBError> {
    let mut backend = postgres.get().await.map_err(DBError::PGPoolError)?;
    log::debug!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
    let client = &mut **backend;
    embedded::migrations::runner().run_async(client).await?;
    Ok(())
}
