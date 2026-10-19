use crate::models::HubError;
use crate::repositories::hub_registry::{redis::RedisHubConnectionDbContext, HubConnection, HubRegistry};
use redis::{AsyncCommands, Script};
use shine_infra::db::redis::RedisError;
use std::sync::LazyLock;
use tracing::instrument;
use uuid::Uuid;

const HUB_CONNECTION_KEYSPACE: &str = "hub-connection:";
pub const HUB_REGISTRY_CHANGED_CHANNEL: &str = "hub-registry-changed";

/// Registry entries per pipelined round trip of the batched heartbeat.
const HEARTBEAT_CHUNK: usize = 256;

/// Body shared by the single and the batched heartbeat: refresh the TTL when the entry still holds
/// our connection id, re-create it when it is missing (a flush, a failover or an expiry during an
/// outage left no competing owner) and report a foreign owner. `KEYS[1]` is the entry, `ARGV[1]`
/// our connection id and `ARGV[2]` the TTL. Returns 1 when the entry is ours, 0 otherwise.
const HEARTBEAT_LUA: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then
    redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
    return 1
end
if current == ARGV[1] then
    redis.call('EXPIRE', KEYS[1], ARGV[2])
    return 1
end
return 0
"#;

/// Deletes the entry when it still holds our connection id and announces the change in the same
/// atomic step. `KEYS[1]` is the entry, `ARGV[1]` our connection id, `ARGV[2]` the channel and
/// `ARGV[3]` the payload. Returns 1 when removed, 0 otherwise.
const REMOVE_LUA: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    redis.call('DEL', KEYS[1])
    redis.call('PUBLISH', ARGV[2], ARGV[3])
    return 1
end
return 0
"#;

static HEARTBEAT_SCRIPT: LazyLock<Script> = LazyLock::new(|| Script::new(HEARTBEAT_LUA));
static REMOVE_SCRIPT: LazyLock<Script> = LazyLock::new(|| Script::new(REMOVE_LUA));

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
            .map_err(RedisError::RawError)?;

        while let Some(key) = iter.next_item().await {
            keys.push(key.map_err(RedisError::RawError)?);
        }

        Ok(keys)
    }
}

impl HubRegistry for RedisHubConnectionDbContext<'_> {
    #[instrument(level = "debug", skip_all, fields(user_id = %user_id, ws.connection_id = %connection_id))]
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

    #[instrument(level = "debug", skip_all, fields(user_id = %user_id, ws.connection_id = %connection_id))]
    async fn heartbeat_connection(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let key = self.to_redis_key(user_id);
        let owned: i64 = HEARTBEAT_SCRIPT
            .key(&key)
            .arg(connection_id.to_string())
            .arg(self.ttl_seconds)
            .invoke_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)?;

        Ok(owned == 1)
    }

    #[instrument(level = "debug", skip_all, fields(count = connections.len()))]
    async fn heartbeat_connections(&mut self, connections: &[HubConnection]) -> Result<Vec<HubConnection>, HubError> {
        let mut stale = Vec::new();

        // One pipelined round trip per chunk keeps a single command from growing with the number of
        // connections. Each entry is checked atomically by the script, so there is no window between
        // reading an entry and refreshing it.
        for chunk in connections.chunks(HEARTBEAT_CHUNK) {
            let mut pipe = redis::pipe();
            for connection in chunk {
                pipe.cmd("EVAL")
                    .arg(HEARTBEAT_LUA)
                    .arg(1)
                    .arg(self.to_redis_key(connection.user_id))
                    .arg(connection.connection_id.to_string())
                    .arg(self.ttl_seconds);
            }

            let owned: Vec<i64> = pipe
                .query_async(&mut *self.client)
                .await
                .map_err(RedisError::RawError)?;

            stale.extend(chunk.iter().zip(owned).filter(|(_, owned)| *owned != 1).map(|(c, _)| *c));
        }

        Ok(stale)
    }

    #[instrument(level = "debug", skip_all)]
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

    #[instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    async fn find_connection_by_user(&mut self, user_id: Uuid) -> Result<Option<HubConnection>, HubError> {
        let key = self.to_redis_key(user_id);
        let connection: Option<String> = self.client.get(&key).await.map_err(RedisError::RawError)?;
        let connection_id = self.parse_connection_id(connection);

        Ok(connection_id.map(|connection_id| HubConnection { user_id, connection_id }))
    }

    #[instrument(level = "debug", skip_all, fields(user_id = %user_id, ws.connection_id = %connection_id))]
    async fn remove_connection_if_active(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let key = self.to_redis_key(user_id);
        let removed: i64 = REMOVE_SCRIPT
            .key(&key)
            .arg(connection_id.to_string())
            .arg(HUB_REGISTRY_CHANGED_CHANNEL)
            .arg(self.to_change_payload(user_id))
            .invoke_async(&mut *self.client)
            .await
            .map_err(RedisError::RawError)?;

        Ok(removed == 1)
    }
}
