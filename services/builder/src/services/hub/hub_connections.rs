use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, ToTopic, TopicKey},
    services::hub::hub_metrics::hub_metrics,
};
use shine_infra::{session::SessionKey, telemetry::label};
use std::collections::HashMap;
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    RwLock,
};
use uuid::Uuid;

struct Connection {
    user_id: Uuid,
    session_key: SessionKey,
    tx: mpsc::Sender<HubMessage>,
    topics: Vec<TopicKey>,
}

#[derive(Default)]
struct Registry {
    /// user_id → its current connection_id.
    users: HashMap<Uuid, Uuid>,
    /// connection_id → the connection's channel and topics.
    connections: HashMap<Uuid, Connection>,
}

/// A connection found dead during a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadConnection {
    pub connection_id: Uuid,
    pub reason: DisconnectReason,
}

/// A connection taken out of the registry. Dropping it releases the egress channel, which ends the socket.
pub struct RemovedConnection {
    pub user_id: Uuid,
    pub connection_id: Uuid,
    tx: mpsc::Sender<HubMessage>,
}

impl RemovedConnection {
    /// Best-effort delivery of the event that ends the connection, as it is no longer registered to
    /// receive broadcasts; a full or closed channel is ignored.
    pub fn notify(self, event: HubEvent) {
        let _ = try_send_as(&self.tx, HubMessage::Hub(event), "close");
    }
}

/// Non-blocking send to a connection, recording the outcome. `Err` carries the drop reason of a dead connection.
fn try_send(tx: &mpsc::Sender<HubMessage>, message: HubMessage) -> Result<(), DisconnectReason> {
    let kind = match &message {
        HubMessage::Hub(_) => "hub",
        HubMessage::Chat(_) => "chat",
    };
    try_send_as(tx, message, kind)
}

fn try_send_as(tx: &mpsc::Sender<HubMessage>, message: HubMessage, kind: &'static str) -> Result<(), DisconnectReason> {
    let (outcome, result) = match tx.try_send(message) {
        Ok(()) => ("ok", Ok(())),
        Err(TrySendError::Full(_)) => ("full", Err(DisconnectReason::SlowConsumer)),
        Err(TrySendError::Closed(_)) => ("closed", Err(DisconnectReason::DeadChannel)),
    };
    hub_metrics()
        .messages_sent
        .add(1, &[label("kind", kind), label("outcome", outcome)]);
    result
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
        tx: mpsc::Sender<HubMessage>,
        topics: Vec<TopicKey>,
    ) {
        let mut registry = self.registry.write().await;
        registry.users.insert(user_id, connection_id);
        let replaced = registry
            .connections
            .insert(connection_id, Connection { user_id, session_key, tx, topics });
        if replaced.is_none() {
            hub_metrics().connections.add(1, &[]);
        }
    }

    /// Removes a user's connection, only if it is still `connection_id` when given.
    pub async fn remove_connection(&self, user_id: Uuid, connection_id: Option<Uuid>) -> Option<RemovedConnection> {
        let mut registry = self.registry.write().await;
        if let Some(expected) = connection_id {
            if registry.users.get(&user_id).copied() != Some(expected) {
                return None;
            }
        }
        let removed = registry.users.remove(&user_id)?;
        Self::take(&mut registry, user_id, removed)
    }

    /// Removes a connection by its id.
    pub async fn remove_connection_by_id(&self, connection_id: Uuid) -> Option<RemovedConnection> {
        let mut registry = self.registry.write().await;
        let user_id = registry.connections.get(&connection_id)?.user_id;
        // A superseded connection may still be registered by id while the user points at its successor.
        if registry.users.get(&user_id) == Some(&connection_id) {
            registry.users.remove(&user_id);
        }
        Self::take(&mut registry, user_id, connection_id)
    }

    /// Removes every connection.
    pub async fn remove_all(&self) -> Vec<RemovedConnection> {
        let mut registry = self.registry.write().await;
        let users: Vec<_> = registry.users.drain().collect();
        users
            .into_iter()
            .filter_map(|(user_id, connection_id)| Self::take(&mut registry, user_id, connection_id))
            .collect()
    }

    fn take(registry: &mut Registry, user_id: Uuid, connection_id: Uuid) -> Option<RemovedConnection> {
        let connection = registry.connections.remove(&connection_id)?;
        hub_metrics().connections.add(-1, &[]);
        Some(RemovedConnection {
            user_id,
            connection_id,
            tx: connection.tx,
        })
    }

    /// The user's active `(connection_id, session_key)`, when connected.
    pub async fn find_active(&self, user_id: Uuid) -> Option<(Uuid, SessionKey)> {
        let registry = self.registry.read().await;
        let connection_id = *registry.users.get(&user_id)?;
        let data = registry.connections.get(&connection_id)?;
        Some((connection_id, data.session_key))
    }

    /// Sends a message to one connection. `Ok(false)` if the connection is unknown, `Err` carries the dead
    /// connection to prune.
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<bool, DeadConnection> {
        let registry = self.registry.read().await;
        let Some(data) = registry.connections.get(&connection_id) else {
            return Ok(false);
        };
        if !data.topics.contains(&message.topic()) {
            return Ok(false);
        }
        try_send(&data.tx, message)
            .map(|()| true)
            .map_err(|reason| DeadConnection { connection_id, reason })
    }

    /// Sends a message to the user's active connection. `Err` carries the dead connection to prune.
    #[allow(dead_code)]
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), DeadConnection> {
        let registry = self.registry.read().await;
        let Some(&connection_id) = registry.users.get(&user_id) else {
            return Ok(());
        };
        let Some(data) = registry.connections.get(&connection_id) else {
            return Ok(());
        };
        if !data.topics.contains(&message.topic()) {
            return Ok(());
        }
        try_send(&data.tx, message).map_err(|reason| DeadConnection { connection_id, reason })
    }

    /// Broadcasts a message to every topic-matching connection. `Err` carries the dead connections to prune.
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), Vec<DeadConnection>> {
        let topic = message.topic();
        let mut dead = Vec::new();

        let registry = self.registry.read().await;
        for (connection_id, data) in registry.connections.iter() {
            if !data.topics.contains(&topic) {
                continue;
            }
            if let Err(reason) = try_send(&data.tx, message.clone()) {
                dead.push(DeadConnection {
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

    /// Every active `(user_id, connection_id)`.
    pub async fn snapshot(&self) -> Vec<(Uuid, Uuid)> {
        let registry = self.registry.read().await;
        registry.users.iter().map(|(user_id, connection_id)| (*user_id, *connection_id)).collect()
    }

    /// Whether the user may have a connection here, without waiting: when the registry is being
    /// written the answer is a conservative `true`.
    pub fn may_hold_user(&self, user_id: Uuid) -> bool {
        match self.registry.try_read() {
            Ok(registry) => registry.users.contains_key(&user_id),
            Err(_) => true,
        }
    }

    /// Number of connected users, for status reporting.
    pub async fn len(&self) -> usize {
        self.registry.read().await.users.len()
    }
}

#[cfg(test)]
#[path = "hub_connections_test.rs"]
mod hub_connections_test;
