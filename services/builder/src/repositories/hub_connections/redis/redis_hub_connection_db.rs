use crate::repositories::hub_connections::{
    redis::RedisHubConnectionBuildError, HubConnectionDb, HubConnectionDbContext, HubConnectionError,
};
use shine_infra::db::{DBError, RedisConnectionPool, RedisPooledConnection};

pub struct RedisHubConnectionDbContext<'c> {
    pub(in crate::repositories::hub_connections::redis) client: RedisPooledConnection<'c>,
}

impl<'c> HubConnectionDbContext<'c> for RedisHubConnectionDbContext<'c> {}

#[derive(Clone)]
pub struct RedisHubConnectionDb {
    client: RedisConnectionPool,
}

impl RedisHubConnectionDb {
    pub async fn new(redis: &RedisConnectionPool) -> Result<Self, RedisHubConnectionBuildError> {
        let _client = redis.get().await.map_err(DBError::RedisPoolError)?;

        Ok(Self { client: redis.clone() })
    }
}

impl HubConnectionDb for RedisHubConnectionDb {
    async fn create_context(&self) -> Result<impl HubConnectionDbContext<'_>, HubConnectionError> {
        let client = self.client.get().await.map_err(DBError::RedisPoolError)?;

        Ok(RedisHubConnectionDbContext { client })
    }
}
