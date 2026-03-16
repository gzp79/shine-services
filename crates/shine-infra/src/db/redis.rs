use crate::health::StatusProvider;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};

pub use bb8_redis::RedisConnectionManager;
pub use shine_infra_macros::RedisJsonValue;

pub type RedisConnectionError = RunError<<RedisConnectionManager as ManageConnection>::Error>;
pub type RedisConnectionPool = BB8Pool<RedisConnectionManager>;
pub type RedisPooledConnection<'a> = PooledConnection<'a, RedisConnectionManager>;

pub struct RedisPoolStatus {
    pool: RedisConnectionPool,
}

impl RedisPoolStatus {
    pub fn new(pool: RedisConnectionPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StatusProvider for RedisPoolStatus {
    fn name(&self) -> &'static str {
        "redis"
    }

    async fn status(&self) -> serde_json::Value {
        let state = self.pool.state();
        serde_json::json!({
            "connections": state.connections,
            "idleConnections": state.idle_connections
        })
    }
}

pub async fn create_redis_pool(cns: &str) -> Result<RedisConnectionPool, RedisConnectionError> {
    // Parse connection string
    // Format: redis://host:port?timeout=3000&pool_timeout=5000
    // - timeout: Redis native parameter in MILLISECONDS (TCP connection and command timeout)
    // - pool_timeout: custom parameter in MILLISECONDS for bb8 pool (acquiring connection from pool, including waiting for connection to be established if pool is exhausted)

    let (pool_timeout_opt, cns_clean) = crate::db::extract_and_strip_param(cns, "pool_timeout");
    let pool_timeout_ms = pool_timeout_opt.unwrap_or(30000); // Default: 30s

    let redis_manager = RedisConnectionManager::new(cns_clean)?;
    let redis = bb8::Pool::builder()
        .max_size(10) // Set the maximum number of connections in the pool
        .connection_timeout(std::time::Duration::from_millis(pool_timeout_ms))
        .build(redis_manager)
        .await?;

    {
        let client = &mut *redis.get().await?;
        let pong: String = redis::cmd("PING").query_async(client).await?;
        log::info!("Redis pong: {pong}");
    }

    Ok(redis)
}
