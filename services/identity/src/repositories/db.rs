use serde::{Deserialize, Serialize};
use shine_infra::db::{
    postgres::{PgConnectionPool, PgError},
    redis::{RedisConnectionPool, RedisError},
};

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

pub async fn create_postgres_pool(stage: &str, config: &DBConfig) -> Result<PgConnectionPool, PgError> {
    let postgres = shine_infra::db::postgres::create_postgres_pool(stage, config.sql_cns.as_str())
        .await
        .map_err(PgError::CreatePoolError)?;

    migrate_postgres(&postgres).await?;
    Ok(postgres)
}

pub async fn create_redis_pool(stage: &str, config: &DBConfig) -> Result<RedisConnectionPool, RedisError> {
    shine_infra::db::redis::create_redis_pool(stage, config.redis_cns.as_str())
        .await
        .map_err(RedisError::PoolError)
}

async fn migrate_postgres(postgres: &PgConnectionPool) -> Result<(), PgError> {
    let mut backend = postgres.get().await.map_err(PgError::PoolError)?;
    tracing::trace!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
    let report = backend.run_migrations(embedded::migrations::runner()).await?;
    let applied = report
        .applied_migrations()
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();
    tracing::info!("Applied {} postgres migration(s): {applied:?}", applied.len());
    Ok(())
}
