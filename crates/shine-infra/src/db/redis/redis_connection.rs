use super::{RedisListener, RedisListenerError};
use crate::{db::CnsParamError, health::StatusProvider};
use async_trait::async_trait;
use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};
use redis::{
    aio::{ConnectionLike, MultiplexedConnection},
    AsyncConnectionConfig, Client, Cmd, ErrorKind, Pipeline, RedisError, RedisFuture, Value,
};
use std::ops::{Deref, DerefMut};
use std::time::Duration;

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
        F: Fn(Option<&str>) + Send + Sync + 'static,
    {
        self.listener.listen(channel, handler).await
    }

    #[inline]
    pub async fn unlisten(&self, channel: &str) -> Result<(), RedisListenerError> {
        self.listener.unlisten(channel).await
    }

    #[inline]
    pub async fn unlisten_all(&self) -> Result<(), RedisListenerError> {
        self.listener.unlisten_all().await
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
    /// Per-connection connect/response timeouts from the `timeout` CNS param. redis-rs does not read
    /// `timeout` from the URL, so it must be applied here explicitly (see `create_redis_pool`).
    conn_config: AsyncConnectionConfig,
    listener: RedisListener,
}

impl RedisConnectionManager {
    pub fn new(
        raw_cns: &str,
        conn_config: AsyncConnectionConfig,
        connect_timeout: Duration,
        max_reconnect_backoff: Duration,
    ) -> Result<Self, RedisError> {
        let client = Client::open(raw_cns)?;
        let listener = RedisListener::new(client.clone(), connect_timeout, max_reconnect_backoff);
        Ok(Self { client, conn_config, listener })
    }
}

impl ManageConnection for RedisConnectionManager {
    type Connection = RedisConnection;
    type Error = RedisError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let client = self
            .client
            .get_multiplexed_async_connection_with_config(&self.conn_config)
            .await?;
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

/// Format: `redis://host:port?timeout=3000&pool_timeout=5000&max_size=10`
/// - `timeout`: custom parameter in MILLISECONDS applied to the redis connection as both the
///   connect and per-command response timeout (default: redis-rs defaults, 1s connect / 500ms response)
/// - `pool_timeout`: custom parameter in MILLISECONDS for bb8 pool (acquiring connection from pool, including waiting for connection to be established if pool is exhausted)
/// - `max_size`: custom parameter for the maximum number of pooled connections (default 10)
pub async fn create_redis_pool(cns: &str) -> Result<RedisConnectionPool, RedisConnectionError> {
    let to_redis_err = |e: CnsParamError| {
        RunError::User(RedisError::from((
            ErrorKind::InvalidClientConfig,
            "invalid connection string parameter",
            e.to_string(),
        )))
    };
    let invalid_param = |name: &'static str| -> RedisConnectionError {
        RunError::User(RedisError::from((
            ErrorKind::InvalidClientConfig,
            "connection string parameter must be greater than zero",
            name.to_string(),
        )))
    };

    let mut cns = crate::db::ConnectionString::parse(cns);
    let pool_timeout_ms = cns.take_u64("pool_timeout").map_err(to_redis_err)?.unwrap_or(30000);
    let max_size = cns.take_u64("max_size").map_err(to_redis_err)?.unwrap_or(10);
    // `timeout` is NOT a redis-rs URL parameter: it is collected and never read, so it must be
    // applied to the connection config here or every command silently stays at redis-rs's 500 ms
    // default response timeout. Absent → keep redis-rs defaults (500 ms response, 1 s connect).
    let timeout_ms = cns.take_u64("timeout").map_err(to_redis_err)?;
    let cns_clean = cns.into_cns();

    // Reject the degenerate values bb8/redis would otherwise accept: max_size=0 asserts inside bb8
    // (a startup panic), pool_timeout=0 times out every checkout immediately, and timeout=0 times
    // out every command immediately.
    if max_size == 0 || max_size > u32::MAX as u64 {
        return Err(invalid_param("max_size"));
    }
    if pool_timeout_ms == 0 {
        return Err(invalid_param("pool_timeout"));
    }
    if timeout_ms == Some(0) {
        return Err(invalid_param("timeout"));
    }
    let max_size = max_size as u32;
    let pool_timeout = std::time::Duration::from_millis(pool_timeout_ms);

    let mut conn_config = AsyncConnectionConfig::new();
    // The listener's pub/sub path ignores redis-rs's connect/response timeouts, so it takes an
    // explicit connect timeout: the `timeout` param if set, else a sane default.
    let mut listener_connect_timeout = RedisListener::DEFAULT_CONNECT_TIMEOUT;
    if let Some(timeout_ms) = timeout_ms {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        conn_config = conn_config
            .set_connection_timeout(Some(timeout))
            .set_response_timeout(Some(timeout));
        listener_connect_timeout = timeout;
    }

    let redis_manager = RedisConnectionManager::new(&cns_clean, conn_config, listener_connect_timeout, pool_timeout)?;
    let redis = bb8::Pool::builder()
        .max_size(max_size)
        .connection_timeout(pool_timeout)
        .build(redis_manager)
        .await?;

    Ok(redis)
}

#[cfg(test)]
mod test {
    use super::*;

    // Degenerate pool params are rejected before any network I/O, so these run without a server.
    fn err_kind(err: RedisConnectionError) -> ErrorKind {
        match err {
            RunError::User(e) => e.kind(),
            RunError::TimedOut => panic!("expected a config error, got a pool timeout"),
        }
    }

    #[tokio::test]
    async fn max_size_zero_is_rejected_not_panicked() {
        let err = create_redis_pool("redis://localhost?max_size=0").await.unwrap_err();
        assert_eq!(err_kind(err), ErrorKind::InvalidClientConfig);
    }

    #[tokio::test]
    async fn max_size_overflowing_u32_is_rejected() {
        let cns = format!("redis://localhost?max_size={}", u32::MAX as u64 + 1);
        let err = create_redis_pool(&cns).await.unwrap_err();
        assert_eq!(err_kind(err), ErrorKind::InvalidClientConfig);
    }

    #[tokio::test]
    async fn pool_timeout_zero_is_rejected() {
        let err = create_redis_pool("redis://localhost?pool_timeout=0").await.unwrap_err();
        assert_eq!(err_kind(err), ErrorKind::InvalidClientConfig);
    }

    #[tokio::test]
    async fn timeout_zero_is_rejected() {
        let err = create_redis_pool("redis://localhost?timeout=0").await.unwrap_err();
        assert_eq!(err_kind(err), ErrorKind::InvalidClientConfig);
    }
}
