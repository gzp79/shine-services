use crate::repositories::hub_registry::HubConnectionError;
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
    ) -> impl Future<Output = Result<(), HubConnectionError>> + Send;

    /// Extends the heartbeat TTL for the user only when the provided connection id matches
    /// the currently active one.
    ///
    /// Returns `true` when TTL was extended, `false` when no matching active connection exists.
    fn heartbeat_connection(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<bool, HubConnectionError>> + Send;

    /// Batched heartbeat for a set of locally-tracked connections. Extends the TTL of every
    /// registry entry that still holds the provided connection id, and reports the connections
    /// the registry no longer holds as active so the caller can disconnect them.
    ///
    /// The TTL is only a crash-cleanup backstop, so refreshing it is never harmful even if the
    /// entry now holds a newer connection; disconnect decisions rely on the id comparison, which
    /// is also covered by the pub/sub path and the next tick.
    ///
    /// Returns the subset of `connections` whose registry entry is missing or holds a different
    /// connection id (i.e. the ones to disconnect locally).
    fn heartbeat_connections(
        &mut self,
        connections: &[HubConnection],
    ) -> impl Future<Output = Result<Vec<HubConnection>, HubConnectionError>> + Send;

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
