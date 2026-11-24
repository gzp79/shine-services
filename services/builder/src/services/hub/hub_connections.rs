use crate::{
    models::messages::{DisconnectReason, HubMessage, ToTopic, TopicKey},
    services::hub::{hub_metrics::hub_metrics, socket_channel::SocketSender},
};
use shine_infra::session::SessionKey;
use std::collections::HashMap;
use tokio::sync::{mpsc::error::TrySendError, RwLock};
use uuid::Uuid;

/// A connection found dead during a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadConnection {
    pub user_id: Uuid,
    pub connection_id: Uuid,
    pub reason: DisconnectReason,
}

/// A connection taken out of the registry. Every removal has to end it with [`RemovedConnection::close`], which
/// tells the socket why; dropping it without a reason is an error.
#[must_use = "a removed connection has to be ended with close(reason)"]
pub struct RemovedConnection {
    pub user_id: Uuid,
    pub connection_id: Uuid,
    socket_sender: SocketSender,
    closed: bool,
}

impl RemovedConnection {
    /// Ends the connection with the reason, unless something already did. Returns the reason the connection
    /// ended with, which is the given one only if nothing ended it first.
    pub fn close(mut self, reason: DisconnectReason) -> DisconnectReason {
        self.closed = true;
        self.socket_sender.close(reason);
        self.socket_sender.close_reason().unwrap_or(reason)
    }
}

impl Drop for RemovedConnection {
    fn drop(&mut self) {
        if !self.closed {
            tracing::error!(
                "[{}] [{}] Connection removed without a reason",
                self.user_id,
                self.connection_id
            );
        }
    }
}

struct Connection {
    user_id: Uuid,
    session_key: SessionKey,
    socket_sender: SocketSender,
    topics: Vec<TopicKey>,
}

#[derive(Default)]
struct Registry {
    /// user_id → its current connection_id.
    users: HashMap<Uuid, Uuid>,
    /// connection_id → the connection's channel and topics.
    connections: HashMap<Uuid, Connection>,
}

/// The registry of addressable, live user connections.
#[derive(Default)]
pub struct Connections {
    registry: RwLock<Registry>,
}

impl Connections {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a bounded egress channel as a user's connection.
    pub async fn register_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        session_key: SessionKey,
        socket_sender: SocketSender,
        topics: Vec<TopicKey>,
    ) {
        let mut registry = self.registry.write().await;
        registry.users.insert(user_id, connection_id);
        let replaced = registry.connections.insert(
            connection_id,
            Connection {
                user_id,
                session_key,
                socket_sender,
                topics,
            },
        );
        if replaced.is_none() {
            hub_metrics().connection_added();
        }
    }

    /// Removes a user's connection, only if it is still `connection_id` when given.
    #[must_use = "a removed connection has to be ended with close(reason)"]
    pub async fn unregister_connection(&self, user_id: Uuid, connection_id: Option<Uuid>) -> Option<RemovedConnection> {
        let mut registry = self.registry.write().await;
        if let Some(expected) = connection_id {
            if registry.users.get(&user_id).copied() != Some(expected) {
                return None;
            }
        }
        let removed = registry.users.remove(&user_id)?;
        Self::take(&mut registry, user_id, removed)
    }

    /// Removes every connection.
    #[must_use = "a removed connection has to be ended with close(reason)"]
    pub async fn unregister_all(&self) -> Vec<RemovedConnection> {
        let mut registry = self.registry.write().await;
        let users: Vec<_> = registry.users.drain().collect();
        users
            .into_iter()
            .filter_map(|(user_id, connection_id)| Self::take(&mut registry, user_id, connection_id))
            .collect()
    }

    fn take(registry: &mut Registry, user_id: Uuid, connection_id: Uuid) -> Option<RemovedConnection> {
        let connection = registry.connections.remove(&connection_id)?;
        hub_metrics().connection_removed();
        Some(RemovedConnection {
            user_id,
            connection_id,
            socket_sender: connection.socket_sender,
            closed: false,
        })
    }

    /// The user's active `(connection_id, session_key)`, when connected.
    pub async fn find_active(&self, user_id: Uuid) -> Option<(Uuid, SessionKey)> {
        let registry = self.registry.read().await;
        let connection_id = *registry.users.get(&user_id)?;
        let data = registry.connections.get(&connection_id)?;
        Some((connection_id, data.session_key))
    }

    /// Sends a message to one connection. `Ok(false)` if the connection is unknown or already ended, `Err` carries
    /// the dead connection, to the first send that found it.
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<bool, DeadConnection> {
        let registry = self.registry.read().await;
        let Some(data) = registry.connections.get(&connection_id) else {
            return Ok(false);
        };
        match try_send(&data.socket_sender, message) {
            Sent::Queued => Ok(true),
            Sent::Closed => Ok(false),
            Sent::Failed(reason) => Err(DeadConnection {
                user_id: data.user_id,
                connection_id,
                reason,
            }),
        }
    }

    /// Sends a message to the user's active connection. `Err` carries the dead connection, to the first send that
    /// found it.
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), DeadConnection> {
        let registry = self.registry.read().await;
        let Some(&connection_id) = registry.users.get(&user_id) else {
            return Ok(());
        };
        let Some(data) = registry.connections.get(&connection_id) else {
            return Ok(());
        };
        match try_send(&data.socket_sender, message) {
            Sent::Queued | Sent::Closed => Ok(()),
            Sent::Failed(reason) => Err(DeadConnection { user_id, connection_id, reason }),
        }
    }

    /// Broadcasts a message to every topic-matching connection. `Err` carries the dead connections, each to the
    /// first send that found it.
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), Vec<DeadConnection>> {
        let topic = message.topic();
        let mut dead = Vec::new();

        let registry = self.registry.read().await;
        for (connection_id, data) in registry.connections.iter() {
            if !data.topics.contains(&topic) {
                continue;
            }
            if let Sent::Failed(reason) = try_send(&data.socket_sender, message.clone()) {
                dead.push(DeadConnection {
                    user_id: data.user_id,
                    connection_id: *connection_id,
                    reason,
                });
            }
        }

        if dead.is_empty() {
            Ok(())
        } else {
            Err(dead)
        }
    }

    /// Whether the connection is registered.
    pub async fn contains(&self, connection_id: Uuid) -> bool {
        self.registry.read().await.connections.contains_key(&connection_id)
    }

    /// Number of connected users, for status reporting.
    pub async fn len(&self) -> usize {
        self.registry.read().await.users.len()
    }
}

/// What became of a message sent to a connection.
enum Sent {
    Queued,
    Failed(DisconnectReason),
    Closed,
}

fn try_send(socket_sender: &SocketSender, message: HubMessage) -> Sent {
    let kind = match &message {
        HubMessage::Hub(_) => "hub",
        HubMessage::Ping(_) => "ping",
        HubMessage::StreamMessage(_) | HubMessage::StreamEvent(_) => "stream",
    };
    let fail = |reason| {
        if socket_sender.close(reason) {
            Sent::Failed(reason)
        } else {
            Sent::Closed
        }
    };
    let (outcome, sent) = if socket_sender.close_reason().is_some() {
        ("closed", Sent::Closed)
    } else {
        match socket_sender.try_send(message) {
            Ok(()) => ("ok", Sent::Queued),
            Err(TrySendError::Full(_)) => ("full", fail(DisconnectReason::SlowConsumer)),
            Err(TrySendError::Closed(_)) => ("closed", fail(DisconnectReason::DeadChannel)),
        }
    };
    hub_metrics().message_sent(kind, outcome);
    sent
}

#[cfg(test)]
#[path = "hub_connections_test.rs"]
mod hub_connections_test;
