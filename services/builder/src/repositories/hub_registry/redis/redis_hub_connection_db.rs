use crate::models::HubError;
use crate::repositories::hub_registry::{
    redis::{RedisHubRegistryBuildError, HUB_REGISTRY_CHANGED_CHANNEL},
    HubConnectionDb, HubConnectionDbContext,
};
use shine_infra::db::redis::{RedisConnectionPool, RedisError, RedisPooledConnection};
use uuid::Uuid;

/// A change observed on the registry channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryChange {
    /// Another instance changed the user's entry.
    User(Uuid),
    /// The listener reconnected: notices published during the outage are lost, so every local
    /// connection must be rechecked against the registry.
    Resync,
}

pub struct RedisHubConnectionDbContext<'c> {
    pub(in crate::repositories::hub_registry::redis) client: RedisPooledConnection<'c>,
    pub(in crate::repositories::hub_registry::redis) ttl_seconds: u64,
    /// Identity of this process, stamped onto every published registry change so listeners can
    /// tell self-originated notifications apart. See [`RedisHubConnectionDb::instance_id`].
    pub(in crate::repositories::hub_registry::redis) instance_id: Uuid,
}

impl<'c> HubConnectionDbContext<'c> for RedisHubConnectionDbContext<'c> {}

#[derive(Clone)]
pub struct RedisHubConnectionDb {
    client: RedisConnectionPool,
    ttl_seconds: u64,
    /// A per-process id minted at construction.
    instance_id: Uuid,
}

impl RedisHubConnectionDb {
    pub async fn new(redis: &RedisConnectionPool, ttl_seconds: u64) -> Result<Self, RedisHubRegistryBuildError> {
        let _client = redis.get().await.map_err(RedisError::PoolError)?;

        Ok(Self {
            client: redis.clone(),
            ttl_seconds,
            instance_id: Uuid::new_v4(),
        })
    }

    /// Listens for registry changes.
    pub async fn listen_to_registry_changes<F>(&self, handler: F) -> Result<(), HubError>
    where
        F: Fn(RegistryChange) + Send + Sync + 'static,
    {
        let self_id = self.instance_id.to_string();
        let client = self.client.get().await.map_err(RedisError::PoolError)?;
        client
            .listen(HUB_REGISTRY_CHANGED_CHANNEL, move |payload| {
                let Some(payload) = payload else {
                    handler(RegistryChange::Resync);
                    return;
                };

                // Payload is "{instance_id}:{user_id}".
                let Some((origin, raw_user_id)) = payload.split_once(':') else {
                    tracing::error!("Malformed hub registry payload {payload:?}");
                    return;
                };

                // Drop what this process published itself: its local view is already up to date,
                // so re-checking would only waste a pooled connection and a Redis round trip.
                if origin == self_id {
                    return;
                }

                match Uuid::parse_str(raw_user_id) {
                    Ok(user_id) => handler(RegistryChange::User(user_id)),
                    Err(err) => tracing::error!("Failed to parse hub registry user id {raw_user_id:?}: {err:#?}"),
                }
            })
            .await?;
        Ok(())
    }

    /// Stops listening for registry changes.
    pub async fn unlisten_registry_changes(&self) -> Result<(), HubError> {
        let client = self.client.get().await.map_err(RedisError::PoolError)?;
        client.unlisten(HUB_REGISTRY_CHANGED_CHANNEL).await?;
        Ok(())
    }
}

impl HubConnectionDb for RedisHubConnectionDb {
    async fn create_context(&self) -> Result<impl HubConnectionDbContext<'_>, HubError> {
        let client = self.client.get().await.map_err(RedisError::PoolError)?;

        Ok(RedisHubConnectionDbContext {
            client,
            ttl_seconds: self.ttl_seconds,
            instance_id: self.instance_id,
        })
    }
}
