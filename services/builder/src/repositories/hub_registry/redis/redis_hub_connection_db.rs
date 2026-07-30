use crate::repositories::hub_registry::{
    redis::{RedisHubRegistryBuildError, HUB_REGISTRY_CHANGED_CHANNEL},
    HubConnectionDb, HubConnectionDbContext, HubConnectionError,
};
use shine_infra::db::{DBError, RedisConnectionPool, RedisListenerError, RedisPooledConnection};

pub struct RedisHubConnectionDbContext<'c> {
    pub(in crate::repositories::hub_registry::redis) client: RedisPooledConnection<'c>,
    pub(in crate::repositories::hub_registry::redis) ttl_seconds: u64,
}

impl<'c> HubConnectionDbContext<'c> for RedisHubConnectionDbContext<'c> {}

#[derive(Clone)]
pub struct RedisHubConnectionDb {
    client: RedisConnectionPool,
    ttl_seconds: u64,
}

impl RedisHubConnectionDb {
    pub async fn new(redis: &RedisConnectionPool, ttl_seconds: u64) -> Result<Self, RedisHubRegistryBuildError> {
        let _client = redis.get().await.map_err(DBError::RedisPoolError)?;

        Ok(Self {
            client: redis.clone(),
            ttl_seconds,
        })
    }

    pub async fn listen_to_registry_changes<F>(&self, handler: F) -> Result<(), HubConnectionError>
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let client = self.client.get().await.map_err(DBError::RedisPoolError)?;
        client
            .listen(HUB_REGISTRY_CHANGED_CHANNEL, handler)
            .await
            .map_err(|err| match err {
                RedisListenerError::Redis(err) => DBError::RedisError(err),
            })?;
        Ok(())
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
