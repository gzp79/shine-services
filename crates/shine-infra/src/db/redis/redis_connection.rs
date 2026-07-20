use super::{RedisListener, RedisListenerError};
use crate::health::StatusProvider;
use async_trait::async_trait;
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};
use redis::{
    aio::{ConnectionLike, MultiplexedConnection},
    Client, Cmd, ErrorKind, Pipeline, RedisError, RedisFuture, Value,
};
use std::ops::{Deref, DerefMut};

pub use shine_infra_macros::RedisJsonValue;

/// A pooled connection paired with a clone of the manager's `RedisListener` — mirrors
/// `PGConnection`'s relationship to `PGListener`. Regular commands go through the
/// `MultiplexedConnection` (via `ConnectionLike`/`Deref`/`DerefMut`); `listen` delegates
/// to the listener, which owns its own dedicated connection, independent of this pooled one.
pub struct RedisConnection {
    listener: RedisListener,
    client: MultiplexedConnection,
}

impl RedisConnection {
    #[inline]
    pub async fn listen<F>(&self, channel: &str, handler: F) -> Result<(), RedisListenerError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.listener.listen(channel, handler).await
    }

    #[inline]
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        self.listener.unlisten(channel).await
    }
}

impl ConnectionLike for RedisConnection {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a Cmd) -> RedisFuture<'a, Value> {
        self.client.req_packed_command(cmd)
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        self.client.req_packed_commands(cmd, offset, count)
    }

    fn get_db(&self) -> i64 {
        self.client.get_db()
    }
}

impl Deref for RedisConnection {
    type Target = MultiplexedConnection;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for RedisConnection {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

pub struct RedisConnectionManager {
    client: Client,
    listener: RedisListener,
}

impl RedisConnectionManager {
    pub fn new(raw_cns: &str) -> Result<Self, RedisError> {
        let client = Client::open(raw_cns)?;
        let listener = RedisListener::new(raw_cns)?;
        Ok(Self { client, listener })
    }
}

impl ManageConnection for RedisConnectionManager {
    type Connection = RedisConnection;
    type Error = RedisError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let client = self.client.get_multiplexed_async_connection().await?;
        Ok(RedisConnection {
            listener: self.listener.clone(),
            client,
        })
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let pong: String = redis::cmd("PING").query_async(&mut conn.client).await?;
        match pong.as_str() {
            "PONG" => Ok(()),
            _ => Err((ErrorKind::Extension, "ping request").into()),
        }
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}

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

    let redis_manager = RedisConnectionManager::new(&cns_clean)?;
    let redis = bb8::Pool::builder()
        .max_size(10)
        .connection_timeout(std::time::Duration::from_millis(pool_timeout_ms))
        .build(redis_manager)
        .await?;

    {
        let mut client = redis.get().await?;
        let pong: String = redis::cmd("PING").query_async(&mut *client).await?;
        log::info!("Redis pong: {pong}");
    }

    Ok(redis)
}
