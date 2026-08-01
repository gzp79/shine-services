use crate::repositories::hub_registry::{
    redis::{RedisHubRegistryBuildError, HUB_REGISTRY_CHANGED_CHANNEL},
    HubConnectionDb, HubConnectionDbContext, HubConnectionError,
};
use shine_infra::db::{
    redis::{RedisConnectionPool, RedisListenerError, RedisPooledConnection},
    DBError,
};
use uuid::Uuid;

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
        let _client = redis.get().await.map_err(DBError::RedisPoolError)?;

        Ok(Self {
            client: redis.clone(),
            ttl_seconds,
            instance_id: Uuid::new_v4(),
        })
    }

    /// Listens for registry changes.
    pub async fn listen_to_registry_changes<F>(&self, handler: F) -> Result<(), HubConnectionError>
    where
        F: Fn(Uuid) + Send + Sync + 'static,
    {
        let self_id = self.instance_id.to_string();
        let client = self.client.get().await.map_err(DBError::RedisPoolError)?;
        client
            .listen(HUB_REGISTRY_CHANGED_CHANNEL, move |payload| {
                let Some(payload) = payload else {
                    // None signals a reconnect: pub/sub messages published during the outage were
                    // lost. The periodic registry heartbeat reconciles stale connections, so no
                    // resync is needed here.
                    return;
                };

                // Payload is "{instance_id}:{user_id}".
                let Some((origin, raw_user_id)) = payload.split_once(':') else {
                    log::error!("Malformed hub registry payload {payload:?}");
                    return;
                };

                // Drop what this process published itself: its local view is already up to date,
                // so re-checking would only waste a pooled connection and a Redis round trip.
                if origin == self_id {
                    return;
                }

                match Uuid::parse_str(raw_user_id) {
                    Ok(user_id) => handler(user_id),
                    Err(err) => log::error!("Failed to parse hub registry user id {raw_user_id:?}: {err:#?}"),
                }
            })
            .await
            .map_err(|err| match err {
                RedisListenerError::Redis(err) => DBError::RedisError(err),
                RedisListenerError::Closed => DBError::ListenerClosed,
                RedisListenerError::AlreadyListening(channel) => DBError::AlreadyListening(channel),
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
            instance_id: self.instance_id,
        })
    }
}
