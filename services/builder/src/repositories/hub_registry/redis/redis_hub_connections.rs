use crate::models::HubError;
use crate::repositories::hub_registry::{redis::RedisHubConnectionDbContext, HubConnection, HubConnections};
use redis::{AsyncCommands, Script};
use shine_infra::db::redis::RedisError;
use std::sync::LazyLock;
use tracing::instrument;
use uuid::Uuid;

const HUB_CONNECTION_KEYSPACE: &str = "hub-connection:";
pub const HUB_REGISTRY_CHANGED_CHANNEL: &str = "hub-registry-changed";

/// Rows checked by one call of the heartbeat script: it blocks Redis while it runs.
const HEARTBEAT_CHUNK: usize = 256;

/// Compare and set on the rows, in one atomic step.
///
/// `KEYS` are the rows, `ARGV[1]` the TTL and `ARGV[i + 1]` the connection id that `KEYS[i]` has to hold. A row
/// that holds it gets a new TTL. A row that is gone or holds another connection is left alone: Redis is the truth
/// about who holds the user, and a row is only ever written by a connect. Returns 1 for the rows that are ours,
/// 0 for the others.
static HEARTBEAT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local result = {}
for i, key in ipairs(KEYS) do
    if redis.call('GET', key) == ARGV[i + 1] then
        redis.call('EXPIRE', key, ARGV[1])
        result[i] = 1
    else
        result[i] = 0
    end
end
return result
"#,
    )
});

/// Deletes the row if it holds the connection, and tells the other nodes. `KEYS[1]` is the row, `ARGV[1]` the
/// connection id, `ARGV[2]` the channel and `ARGV[3]` the payload. Returns 1 if it deleted the row.
static REMOVE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    redis.call('DEL', KEYS[1])
    redis.call('PUBLISH', ARGV[2], ARGV[3])
    return 1
end
return 0
"#,
    )
});

/// Redis key of the connection registered for a user.
pub fn hub_connection_key(user_id: Uuid) -> String {
    format!("{HUB_CONNECTION_KEYSPACE}{}", user_id.as_simple())
}

impl RedisHubConnectionDbContext<'_> {
    fn to_redis_key(&self, user_id: Uuid) -> String {
        hub_connection_key(user_id)
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

    /// Runs the heartbeat script over the connections. Whether each of them still holds its row.
    async fn heartbeat_rows(&mut self, connections: &[HubConnection]) -> Result<Vec<bool>, HubError> {
        let mut invocation = HEARTBEAT.prepare_invoke();
        invocation.arg(self.ttl_seconds);
        for connection in connections {
            invocation.key(self.to_redis_key(connection.user_id));
            invocation.arg(connection.connection_id.to_string());
        }
        let held: Vec<i64> = invocation
            .invoke_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)?;
        Ok(held.into_iter().map(|held| held > 0).collect())
    }

    async fn find_redis_keys(&mut self) -> Result<Vec<String>, HubError> {
        let pattern = format!("{HUB_CONNECTION_KEYSPACE}*");
        let mut keys = vec![];
        let mut iter = self
            .client
            .scan_match::<String, _>(pattern)
            .await
            .map_err(RedisError::RawError)?;

        while let Some(key) = iter.next_item().await {
            keys.push(key.map_err(RedisError::RawError)?);
        }

        Ok(keys)
    }
}

impl HubConnections for RedisHubConnectionDbContext<'_> {
    #[instrument(level = "info", skip_all, fields(user_id = %user_id, ws.connection_id = %connection_id))]
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
            .map_err(RedisError::RawError)?;

        Ok(())
    }

    #[instrument(level = "info", skip_all, fields(user_id = %user_id, ws.connection_id = %connection_id))]
    async fn heartbeat_connection(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let held = self.heartbeat_rows(&[HubConnection { user_id, connection_id }]).await?;
        Ok(held.first().copied().unwrap_or(false))
    }

    #[instrument(level = "debug", skip_all, fields(count = connections.len()))]
    async fn heartbeat_connections(&mut self, connections: &[HubConnection]) -> Result<Vec<HubConnection>, HubError> {
        let mut stale = Vec::new();
        for chunk in connections.chunks(HEARTBEAT_CHUNK) {
            let held = self.heartbeat_rows(chunk).await?;
            stale.extend(
                chunk
                    .iter()
                    .zip(held)
                    .filter(|(_, held)| !held)
                    .map(|(connection, _)| *connection),
            );
        }
        Ok(stale)
    }

    #[instrument(level = "info", skip_all)]
    async fn list_connections(&mut self) -> Result<Vec<HubConnection>, HubError> {
        let keys = self.find_redis_keys().await?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let values: Vec<Option<String>> = self.client.mget(keys.clone()).await.map_err(RedisError::RawError)?;
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

    #[instrument(level = "info", skip_all, fields(user_id = %user_id))]
    async fn find_connection_by_user(&mut self, user_id: Uuid) -> Result<Option<HubConnection>, HubError> {
        let key = self.to_redis_key(user_id);
        let connection: Option<String> = self.client.get(&key).await.map_err(RedisError::RawError)?;
        let connection_id = self.parse_connection_id(connection);

        Ok(connection_id.map(|connection_id| HubConnection { user_id, connection_id }))
    }

    #[instrument(level = "info", skip_all, fields(user_id = %user_id, ws.connection_id = %connection_id))]
    async fn remove_connection_if_active(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let removed: i64 = REMOVE
            .key(self.to_redis_key(user_id))
            .arg(connection_id.to_string())
            .arg(HUB_REGISTRY_CHANGED_CHANNEL)
            .arg(self.to_change_payload(user_id))
            .invoke_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)?;
        Ok(removed > 0)
    }
}

#[cfg(test)]
#[path = "redis_hub_connections_test.rs"]
mod redis_hub_connections_test;
