use serde::{Deserialize, Serialize};
use shine_infra::db::{
    postgres::{PgConnectionPool, PgError},
    redis::{RedisConnectionPool, RedisError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub redis_cns: String,
    /// The builder's own Postgres database, for the command log.
    pub sql_cns: String,
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

pub async fn create_redis_pool(config: &DBConfig) -> Result<RedisConnectionPool, RedisError> {
    tracing::info!("Creating redis pool...");
    shine_infra::db::redis::create_redis_pool(config.redis_cns.as_str())
        .await
        .map_err(RedisError::PoolError)
}

pub async fn create_postgres_pool(config: &DBConfig) -> Result<PgConnectionPool, PgError> {
    tracing::info!("Creating postgres pool...");
    let postgres = shine_infra::db::postgres::create_postgres_pool(config.sql_cns.as_str())
        .await
        .map_err(PgError::CreatePoolError)?;

    migrate_postgres(&postgres).await?;
    Ok(postgres)
}

async fn migrate_postgres(postgres: &PgConnectionPool) -> Result<(), PgError> {
    let mut backend = postgres.get().await.map_err(PgError::PoolError)?;
    tracing::trace!("migrations: {:#?}", embedded::migrations::runner().get_migrations());
    let client = &mut **backend;
    let report = embedded::migrations::runner().run_async(client).await?;
    let applied = report
        .applied_migrations()
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();
    tracing::info!("Applied {} postgres migration(s): {applied:?}", applied.len());
    Ok(())
}
