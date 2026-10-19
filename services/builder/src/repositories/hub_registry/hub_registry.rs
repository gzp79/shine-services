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

    /// Extends the heartbeat TTL for the user when the provided connection id is the active one.
    /// A missing entry (flush, failover, expiry during an outage) has no competing owner, so it is
    /// re-created for the provided connection.
    ///
    /// Returns `true` when the connection owns the entry afterwards, `false` when a different
    /// connection owns it.
    fn heartbeat_connection(
        &mut self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<bool, HubError>> + Send;

    /// Batched [`Self::heartbeat_connection`] for a set of locally-tracked connections.
    ///
    /// Returns the subset of `connections` whose registry entry is owned by a different connection
    /// (i.e. the ones to disconnect locally). Missing entries are re-created, not reported.
    fn heartbeat_connections(
        &mut self,
        connections: &[HubConnection],
    ) -> impl Future<Output = Result<Vec<HubConnection>, HubError>> + Send;

    #[allow(dead_code)]
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
