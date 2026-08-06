use serde::{Deserialize, Serialize};
use shine_infra::db::redis::{RedisConnectionPool, RedisError};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub redis_cns: String,
    //pub sql_cns: String,
}

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./sql_migrations");
}

pub async fn create_redis_pool(config: &DBConfig) -> Result<RedisConnectionPool, RedisError> {
    log::info!("Creating redis pool...");
    shine_infra::db::redis::create_redis_pool(config.redis_cns.as_str())
        .await
        .map_err(RedisError::PoolError)
}
