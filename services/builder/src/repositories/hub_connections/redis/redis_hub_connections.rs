use crate::repositories::hub_connections::{
    redis::RedisHubConnectionDbContext, HubConnection, HubConnectionError, HubConnections,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shine_infra::{
    db::{DBError, RedisJsonValue},
    session::{serde_session_key, SessionKey},
};
use uuid::Uuid;

const HUB_CONNECTION_KEYSPACE: &str = "hub-connection:";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
struct RedisHubConnection {
    user_id: Uuid,
    connection_id: Uuid,
    #[serde(with = "serde_session_key")]
    session_key: SessionKey,
}

impl From<HubConnection> for RedisHubConnection {
    fn from(connection: HubConnection) -> Self {
        Self {
            user_id: connection.user_id,
            connection_id: connection.connection_id,
            session_key: connection.session_key,
        }
    }
}

impl From<RedisHubConnection> for HubConnection {
    fn from(connection: RedisHubConnection) -> Self {
        Self {
            user_id: connection.user_id,
            connection_id: connection.connection_id,
            session_key: connection.session_key,
        }
    }
}

impl RedisHubConnectionDbContext<'_> {
    fn to_redis_key(&self, user_id: Uuid) -> String {
        format!("{HUB_CONNECTION_KEYSPACE}user:{}", user_id.as_simple())
    }

    async fn find_redis_keys(&mut self) -> Result<Vec<String>, HubConnectionError> {
        let pattern = format!("{HUB_CONNECTION_KEYSPACE}user:*");
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
    async fn list_connections(&mut self) -> Result<Vec<HubConnection>, HubConnectionError> {
        let keys = self.find_redis_keys().await?;
        let mut connections = Vec::with_capacity(keys.len());

        for key in keys {
            let connection: Option<RedisHubConnection> = self.client.get(&key).await.map_err(DBError::RedisError)?;
            if let Some(connection) = connection {
                connections.push(connection.into());
            }
        }

        Ok(connections)
    }

    async fn create_connection(
        &mut self,
        user_id: Uuid,
        session_key: SessionKey,
    ) -> Result<HubConnection, HubConnectionError> {
        let key = self.to_redis_key(user_id);
        let connection = HubConnection::new(user_id, session_key);
        let redis_connection = RedisHubConnection::from(connection);

        let _: () = self
            .client
            .set(&key, redis_connection)
            .await
            .map_err(DBError::RedisError)?;
        Ok(connection)
    }

    async fn find_connection_by_user(&mut self, user_id: Uuid) -> Result<Option<HubConnection>, HubConnectionError> {
        let key = self.to_redis_key(user_id);
        let connection: Option<RedisHubConnection> = self.client.get(&key).await.map_err(DBError::RedisError)?;
        Ok(connection.map(Into::into))
    }
}
