use crate::repositories::hub_connections::{
    redis::RedisHubConnectionBuildError, HubConnectionDb, HubConnectionDbContext, HubConnectionError,
};
use shine_infra::db::{DBError, RedisConnectionPool, RedisPooledConnection};

pub struct RedisHubConnectionDbContext<'c> {
    pub(in crate::repositories::hub_connections::redis) client: RedisPooledConnection<'c>,
    pub(in crate::repositories::hub_connections::redis) ttl_seconds: u64,
}

impl<'c> HubConnectionDbContext<'c> for RedisHubConnectionDbContext<'c> {}

#[derive(Clone)]
pub struct RedisHubConnectionDb {
    client: RedisConnectionPool,
    ttl_seconds: u64,
}

impl RedisHubConnectionDb {
    pub async fn new(redis: &RedisConnectionPool, ttl_seconds: u64) -> Result<Self, RedisHubConnectionBuildError> {
        let _client = redis.get().await.map_err(DBError::RedisPoolError)?;

        Ok(Self {
            client: redis.clone(),
            ttl_seconds,
        })
    }
}

impl HubConnectionDb for RedisHubConnectionDb {
    async fn create_context(&self) -> Result<impl HubConnectionDbContext<'_>, HubConnectionError> {
        let client = self.client.get().await.map_err(DBError::RedisPoolError)?;

        Ok(RedisHubConnectionDbContext {
            client,
            ttl_seconds: self.ttl_seconds,
        })
    }
}
