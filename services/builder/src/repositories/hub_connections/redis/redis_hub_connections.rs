use crate::repositories::hub_connections::{
    redis::RedisHubConnectionDbContext, HubConnection, HubConnectionError, HubConnections,
};
use redis::{aio::transaction_async, AsyncCommands, SetExpiry, SetOptions};
use shine_infra::db::DBError;
use uuid::Uuid;

const HUB_CONNECTION_KEYSPACE: &str = "hub-connection:";

impl RedisHubConnectionDbContext<'_> {
    fn to_redis_key(&self, user_id: Uuid) -> String {
        format!("{HUB_CONNECTION_KEYSPACE}{}", user_id.as_simple())
    }

    fn user_id_from_redis_key(&self, key: &str) -> Option<Uuid> {
        key.strip_prefix(HUB_CONNECTION_KEYSPACE)
            .and_then(|raw| Uuid::parse_str(raw).ok())
    }

    fn parse_connection_id(&self, value: Option<String>) -> Option<Uuid> {
        value.and_then(|raw| Uuid::parse_str(&raw).ok())
    }

    async fn find_redis_keys(&mut self) -> Result<Vec<String>, HubConnectionError> {
        let pattern = format!("{HUB_CONNECTION_KEYSPACE}*");
        let mut keys = vec![];
        let mut iter = self
            .client
            .scan_match::<String, _>(pattern)
            .await
            .map_err(DBError::RedisError)?;

        while let Some(key) = iter.next_item().await {
            keys.push(key.map_err(DBError::RedisError)?);
        }

        Ok(keys)
    }
}

impl HubConnections for RedisHubConnectionDbContext<'_> {
    async fn create_connection(&mut self, user_id: Uuid) -> Result<Uuid, HubConnectionError> {
        let connection_id = Uuid::new_v4();
        let key = self.to_redis_key(user_id);

        let _: Option<()> = {
            let options = SetOptions::default().with_expiration(SetExpiry::EX(self.ttl_seconds));

            self.client
                .set_options(&key, connection_id.to_string(), options)
                .await
                .map_err(DBError::RedisError)?
        };

        Ok(connection_id)
    }

    async fn heartbeat_connection(&mut self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubConnectionError> {
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

                pipe.expire(&key, ttl_seconds).query_async(&mut con).await
            }
        })
        .await
        .map_err(DBError::RedisError)?;

        Ok(updated)
    }

    async fn list_connections(&mut self) -> Result<Vec<HubConnection>, HubConnectionError> {
        let keys = self.find_redis_keys().await?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let values: Vec<Option<String>> = self.client.mget(keys.clone()).await.map_err(DBError::RedisError)?;
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

    async fn find_connection_by_user(&mut self, user_id: Uuid) -> Result<Option<HubConnection>, HubConnectionError> {
        let key = self.to_redis_key(user_id);
        let connection: Option<String> = self.client.get(&key).await.map_err(DBError::RedisError)?;
        let connection_id = self.parse_connection_id(connection);

        Ok(connection_id.map(|connection_id| HubConnection { user_id, connection_id }))
    }

    async fn remove_connection_if_active(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<bool, HubConnectionError> {
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
        .map_err(DBError::RedisError)?;

        Ok(removed)
    }
}
