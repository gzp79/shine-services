use crate::{
    models::HubError,
    repositories::hub_registry::{redis::RedisHubConnectionDb, HubConnection, HubConnectionDb, HubConnections},
    services::hub::hub_metrics::hub_metrics,
};
use uuid::Uuid;

/// The hub's calls to the cross-instance registry of connections, in Redis. Every failed call is counted in the
/// registry error counter by operation. It is a cheap handle: the command loop and the heartbeat hold one each.
#[derive(Clone)]
pub struct HubRegistry {
    db: RedisHubConnectionDb,
}

impl HubRegistry {
    pub fn new(db: RedisHubConnectionDb) -> Self {
        Self { db }
    }

    /// Makes `connection_id` the active connection of the user, overwriting any other.
    pub async fn create_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let result = async {
            let mut context = self.db.create_context().await?;
            context.create_connection(user_id, connection_id).await
        }
        .await;
        track("create", result)
    }

    /// Whether the registry holds `connection_id` as the user's active connection; `false` on a read error.
    pub async fn is_active(&self, user_id: Uuid, connection_id: Uuid) -> bool {
        let found = match self.db.create_context().await {
            Ok(mut context) => context.find_connection_by_user(user_id).await,
            Err(err) => Err(err),
        };
        match track("find", found) {
            Ok(found) => found.is_some_and(|found| found.connection_id == connection_id),
            Err(err) => {
                tracing::warn!("[{user_id}] Failed to read hub connection: {err:#?}");
                false
            }
        }
    }

    /// Removes the connection, only if it is still the user's active one.
    pub async fn remove_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<(), HubError> {
        let result = async {
            let mut context = self.db.create_context().await?;
            context.remove_connection_if_active(user_id, connection_id).await
        }
        .await;
        track("remove", result).map(|_| ())
    }

    pub async fn heartbeat_connection(&self, user_id: Uuid, connection_id: Uuid) -> Result<bool, HubError> {
        let result = async {
            let mut context = self.db.create_context().await?;
            context.heartbeat_connection(user_id, connection_id).await
        }
        .await;
        track("heartbeat", result)
    }

    /// Batched heartbeat over the given connections, returning the ones the registry no longer holds
    /// as active.
    pub async fn heartbeat_connections(&self, connections: &[HubConnection]) -> Result<Vec<HubConnection>, HubError> {
        let result = async {
            let mut context = self.db.create_context().await?;
            context.heartbeat_connections(connections).await
        }
        .await;
        track("heartbeat", result)
    }

    /// Calls the handler with the user of every change made by another instance.
    pub async fn listen_to_changes<F>(&self, handler: F) -> Result<(), HubError>
    where
        F: Fn(Uuid) + Send + Sync + 'static,
    {
        self.db.listen_to_registry_changes(handler).await
    }

    pub async fn unlisten(&self) -> Result<(), HubError> {
        self.db.unlisten_registry_changes().await
    }
}

/// Counts a failed registry call by operation and passes the result on.
fn track<T>(operation: &'static str, result: Result<T, HubError>) -> Result<T, HubError> {
    if result.is_err() {
        hub_metrics().registry_error(operation);
    }
    result
}
