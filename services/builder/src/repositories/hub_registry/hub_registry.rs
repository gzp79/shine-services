use crate::models::HubError;
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct HubConnection {
    pub user_id: Uuid,
    pub connection_id: Uuid,
}

pub trait HubRegistry {
    /// Creates or replaces the active connection for the user with the given connection id.
    ///
    /// At most one active connection may exist per user.
    fn create_connection(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<(), HubError>> + Send;

    /// Heartbeat for one connection, checked against the registry entry of its user. If the entry holds
    /// the connection id, its TTL is extended. In any other case (the entry is gone, or holds another
    /// connection id) nothing is written.
    ///
    /// Returns `true` when the TTL was extended.
    fn heartbeat_connection(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<bool, HubError>> + Send;

    /// Heartbeat for many locally-tracked connections at once, each as in [`Self::heartbeat_connection`].
    ///
    /// Returns the connections whose TTL was not extended, which the caller disconnects locally.
    fn heartbeat_connections(
        &mut self,
        connections: &[HubConnection],
    ) -> impl Future<Output = Result<Vec<HubConnection>, HubError>> + Send;

    /// Returns all currently connected users with their active connection ids.
    fn list_connections(&mut self) -> impl Future<Output = Result<Vec<HubConnection>, HubError>> + Send;

    /// Returns the active connection for the user, or `None` if the user is not connected.
    fn find_connection_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Option<HubConnection>, HubError>> + Send;

    /// Removes the active connection only when both user and connection id match.
    ///
    /// Returns `true` when removed, `false` when no matching active connection exists.
    fn remove_connection_if_active(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<bool, HubError>> + Send;
}
