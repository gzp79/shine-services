use crate::repositories::hub_connections::HubConnectionError;
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct HubConnection {
    pub user_id: Uuid,
    pub connection_id: Uuid,
}

pub trait HubConnections {
    /// Creates or replaces the active connection for the user and returns the new connection id.
    ///
    /// At most one active connection may exist per user.
    fn create_connection(&mut self, user_id: Uuid) -> impl Future<Output = Result<Uuid, HubConnectionError>> + Send;

    /// Extends the heartbeat TTL for the user only when the provided connection id matches
    /// the currently active one.
    ///
    /// Returns `true` when TTL was extended, `false` when no matching active connection exists.
    fn heartbeat_connection(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<bool, HubConnectionError>> + Send;

    #[allow(dead_code)]
    /// Returns all currently connected users with their active connection ids.
    fn list_connections(&mut self) -> impl Future<Output = Result<Vec<HubConnection>, HubConnectionError>> + Send;

    #[allow(dead_code)]
    /// Returns the active connection for the user, or `None` if the user is not connected.
    fn find_connection_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Option<HubConnection>, HubConnectionError>> + Send;

    /// Removes the active connection only when both user and connection id match.
    ///
    /// Returns `true` when removed, `false` when no matching active connection exists.
    fn remove_connection_if_active(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<bool, HubConnectionError>> + Send;
}
