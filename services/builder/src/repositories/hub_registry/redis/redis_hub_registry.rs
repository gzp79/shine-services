use crate::models::HubError;
use crate::repositories::hub_registry::{redis::RedisHubConnectionDbContext, HubConnection, HubRegistry};
use redis::{aio::transaction_async, AsyncCommands};
use shine_infra::db::redis::RedisDBError;
use uuid::Uuid;

const HUB_CONNECTION_KEYSPACE: &str = "hub-connection:";
pub const HUB_REGISTRY_CHANGED_CHANNEL: &str = "hub-registry-changed";

impl RedisHubConnectionDbContext<'_> {
    fn to_redis_key(&self, user_id: Uuid) -> String {
        format!("{HUB_CONNECTION_KEYSPACE}{}", user_id.as_simple())
    }

    /// Wire payload for the `hub-registry-changed` channel.
    fn to_change_payload(&self, user_id: Uuid) -> String {
        format!("{}:{}", self.instance_id, user_id)
    }

    fn user_id_from_redis_key(&self, key: &str) -> Option<Uuid> {
        key.strip_prefix(HUB_CONNECTION_KEYSPACE)
            .and_then(|raw| Uuid::parse_str(raw).ok())
    }

    fn parse_connection_id(&self, value: Option<String>) -> Option<Uuid> {
        value.and_then(|raw| Uuid::parse_str(&raw).ok())
    }

    async fn find_redis_keys(&mut self) -> Result<Vec<String>, HubError> {
        let pattern = format!("{HUB_CONNECTION_KEYSPACE}*");
        let mut keys = vec![];
        let mut iter = self
            .client
            .scan_match::<String, _>(pattern)
            .await
            .map_err(RedisDBError::RedisError)?;

        while let Some(key) = iter.next_item().await {
            keys.push(key.map_err(RedisDBError::RedisError)?);
        }

        Ok(keys)
    }
}

impl HubRegistry for RedisHubConnectionDbContext<'_> {
    async fn create_connection(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let key = self.to_redis_key(user_id);
        let change_payload = self.to_change_payload(user_id);
        let ttl_seconds = self.ttl_seconds;

        let client = &mut *self.client;

        let _: (usize,) = redis::pipe()
            .cmd("SET")
            .arg(&key)
            .arg(connection_id.to_string())
            .arg("EX")
            .arg(ttl_seconds)
            .ignore()
            .cmd("PUBLISH")
            .arg(HUB_REGISTRY_CHANGED_CHANNEL)
            .arg(change_payload)
            .query_async(client)
            .await
            .map_err(RedisDBError::RedisError)?;

        Ok(())
    }

    async fn heartbeat_connection(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let key = self.to_redis_key(user_id);
        let ttl_seconds = self.ttl_seconds as i64;
        let client = (*self.client).clone();

        let updated = transaction_async(client, &[&key], |mut con, mut pipe| {
            let key = key.clone();
            async move {
                let Some(current): Option<String> = con.get(&key).await? else {
                    return Ok(Some(false));
                };

                let Some(current_connection_id) = Uuid::parse_str(&current).ok() else {
                    return Ok(Some(false));
                };

                if current_connection_id != connection_id {
                    return Ok(Some(false));
                }

                let (updated,): (usize,) = pipe.expire(&key, ttl_seconds).query_async(&mut con).await?;
                Ok(Some(updated > 0))
            }
        })
        .await
        .map_err(RedisDBError::RedisError)?;

        Ok(updated)
    }

    async fn heartbeat_connections(&mut self, connections: &[HubConnection]) -> Result<Vec<HubConnection>, HubError> {
        if connections.is_empty() {
            return Ok(Vec::new());
        }

        // Read every tracked user's current registry entry in one round trip.
        let keys: Vec<String> = connections.iter().map(|c| self.to_redis_key(c.user_id)).collect();
        let current: Vec<Option<String>> = self.client.mget(&keys).await.map_err(RedisDBError::RedisError)?;

        // Compare in memory: entries that still hold our connection id get their TTL refreshed;
        // the rest are reported back so the caller can disconnect them. There is no per-key CAS —
        // a connection replaced between the MGET and the EXPIRE below simply gets a fresh TTL for
        // the newer entry (harmless, TTL is only crash cleanup).
        let mut pipe = redis::pipe();
        let mut refreshed = 0usize;
        let mut stale = Vec::new();

        for ((connection, key), value) in connections.iter().zip(&keys).zip(current) {
            let matches = value
                .as_deref()
                .and_then(|raw| Uuid::parse_str(raw).ok())
                .is_some_and(|id| id == connection.connection_id);

            if matches {
                pipe.expire(key, self.ttl_seconds as i64).ignore();
                refreshed += 1;
            } else {
                stale.push(*connection);
            }
        }

        if refreshed > 0 {
            pipe.query_async::<()>(&mut *self.client)
                .await
                .map_err(RedisDBError::RedisError)?;
        }

        Ok(stale)
    }

    async fn list_connections(&mut self) -> Result<Vec<HubConnection>, HubError> {
        let keys = self.find_redis_keys().await?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let values: Vec<Option<String>> = self.client.mget(keys.clone()).await.map_err(RedisDBError::RedisError)?;
        let mut connections = Vec::with_capacity(values.len());

        for (key, value) in keys.into_iter().zip(values) {
            let Some(user_id) = self.user_id_from_redis_key(&key) else {
                continue;
            };
            let Some(connection_id) = self.parse_connection_id(value) else {
                continue;
            };

            connections.push(HubConnection { user_id, connection_id });
        }

        Ok(connections)
    }

    async fn find_connection_by_user(&mut self, user_id: Uuid) -> Result<Option<HubConnection>, HubError> {
        let key = self.to_redis_key(user_id);
        let connection: Option<String> = self.client.get(&key).await.map_err(RedisDBError::RedisError)?;
        let connection_id = self.parse_connection_id(connection);

        Ok(connection_id.map(|connection_id| HubConnection { user_id, connection_id }))
    }

    async fn remove_connection_if_active(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let key = self.to_redis_key(user_id);
        let client = (*self.client).clone();

        let removed = transaction_async(client, &[&key], |mut con, mut pipe| {
            let key = key.clone();
            async move {
                let Some(current): Option<String> = con.get(&key).await? else {
                    return Ok(Some(false));
                };

                let Some(current_connection_id) = Uuid::parse_str(&current).ok() else {
                    return Ok(Some(false));
                };

                if current_connection_id != connection_id {
                    return Ok(Some(false));
                }

                let (deleted,): (usize,) = pipe.del(&key).query_async(&mut con).await?;
                Ok(Some(deleted > 0))
            }
        })
        .await
        .map_err(RedisDBError::RedisError)?;

        if removed {
            let change_payload = self.to_change_payload(user_id);
            let _: usize = self
                .client
                .publish(HUB_REGISTRY_CHANGED_CHANNEL, change_payload)
                .await
                .map_err(RedisDBError::RedisError)?;
        }

        Ok(removed)
    }
}
