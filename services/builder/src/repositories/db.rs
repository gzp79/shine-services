use anyhow::{anyhow, Error as AnyError};
use serde::{Deserialize, Serialize};
use shine_infra::db::{
    command_log::PgCommandLogDb,
    postgres::{PgConnectionPool, PgError},
    redis::{RedisConnectionPool, RedisError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub redis_cns: String,
    /// The builder's own Postgres database; needed when the command streams are enabled.
    #[serde(default)]
    pub sql_cns: Option<String>,
}

pub async fn create_redis_pool(config: &DBConfig) -> Result<RedisConnectionPool, RedisError> {
    tracing::info!("Creating redis pool...");
    shine_infra::db::redis::create_redis_pool(config.redis_cns.as_str())
        .await
        .map_err(RedisError::PoolError)
}

/// Creates the pool and applies the command-log schema before returning it.
pub async fn create_postgres_pool(config: &DBConfig) -> Result<PgConnectionPool, AnyError> {
    let sql_cns = config
        .sql_cns
        .as_deref()
        .ok_or_else(|| anyhow!("The builder needs db.sqlCns for the command streams"))?;
    tracing::info!("Creating postgres pool...");
    let pool = shine_infra::db::postgres::create_postgres_pool(sql_cns)
        .await
        .map_err(PgError::CreatePoolError)?;
    let mut client = pool.get().await.map_err(PgError::PoolError)?;
    client.migrate("command_log", &PgCommandLogDb::migrations()).await?;
    drop(client);
    Ok(pool)
}

#[cfg(test)]
#[path = "db_test.rs"]
mod db_test;
