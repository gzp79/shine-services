use crate::models::messages::{HubMessage, ToTopic, TopicKey};
use shine_infra::session::SessionKey;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

struct Connection {
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
        registry
            .connections
            .insert(connection_id, Connection { session_key, tx, topics });
    }

    /// Removes a user's connection, returning the removed connection id.
    pub async fn remove_connection(&self, user_id: Uuid, connection_id: Option<Uuid>) -> Option<Uuid> {
        let mut registry = self.registry.write().await;
        if let Some(expected) = connection_id {
            if registry.users.get(&user_id).copied() != Some(expected) {
                return None;
            }
        }
        let removed = registry.users.remove(&user_id)?;
        registry.connections.remove(&removed);
        Some(removed)
    }

    /// Removes a connection by its id, returning `(user_id, connection_id)` when it was present.
    pub async fn remove_connection_by_id(&self, connection_id: Uuid) -> Option<(Uuid, Uuid)> {
        let mut registry = self.registry.write().await;
        let user_id = registry
            .users
            .iter()
            .find(|(_, cid)| **cid == connection_id)
            .map(|(u, _)| *u)?;
        registry.users.remove(&user_id);
        registry.connections.remove(&connection_id);
        Some((user_id, connection_id))
    }

    /// The user's active `(connection_id, session_key)`, when connected.
    pub async fn find_active(&self, user_id: Uuid) -> Option<(Uuid, SessionKey)> {
        let registry = self.registry.read().await;
        let connection_id = *registry.users.get(&user_id)?;
        let data = registry.connections.get(&connection_id)?;
        Some((connection_id, data.session_key))
    }

    /// Sends a message to one connection. `Err` carries the id of a dead connection to prune.
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<(), Uuid> {
        let registry = self.registry.read().await;
        let Some(data) = registry.connections.get(&connection_id) else {
            return Ok(());
        };
        match data.tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(_) => Err(connection_id),
        }
    }

    /// Sends a message to the user's active connection. `Err` carries the id of a dead connection to prune.
    #[allow(dead_code)]
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), Uuid> {
        let registry = self.registry.read().await;
        let Some(&connection_id) = registry.users.get(&user_id) else {
            return Ok(());
        };
        let Some(data) = registry.connections.get(&connection_id) else {
            return Ok(());
        };
        match data.tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(_) => Err(connection_id),
        }
    }

    /// Broadcasts a message to every topic-matching connection. `Err` carries the ids of dead connections to prune.
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), Vec<Uuid>> {
        let topic = message.topic();
        let mut dead = Vec::new();

        let registry = self.registry.read().await;
        for (connection_id, data) in registry.connections.iter() {
            if !data.topics.contains(&topic) {
                continue;
            }
            if data.tx.try_send(message.clone()).is_err() {
                dead.push(*connection_id);
            }
        }

        if dead.is_empty() {
            Ok(())
        } else {
            Err(dead)
        }
    }

    /// Number of connected users, for status reporting.
    pub async fn len(&self) -> usize {
        self.registry.read().await.users.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::messages::{ChatBatch, ChatComment};
    use ring::rand::SystemRandom;
    use shine_infra::session::SessionKey;
    use shine_test::test;
    use tokio::sync::mpsc;

    fn chat(text: &str) -> HubMessage {
        HubMessage::Chat(ChatBatch {
            comments: vec![ChatComment {
                id: "0-0".to_string(),
                user_id: Uuid::new_v4(),
                text: text.to_string(),
            }],
        })
    }

    fn key() -> SessionKey {
        SessionKey::new_random(&SystemRandom::new()).unwrap()
    }

    #[test]
    async fn directed_send_moves_to_the_one_connection() {
        let users = Connections::new();
        let (tx, mut rx) = mpsc::channel(4);
        let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
        users
            .register_connection(user_id, conn_id, key(), tx, vec![TopicKey::Chat])
            .await;

        assert!(users.send_to_connection(conn_id, chat("hi")).await.is_ok());
        assert!(users.send_to_user(user_id, chat("yo")).await.is_ok());

        assert!(matches!(rx.recv().await, Some(HubMessage::Chat(m)) if m.comments[0].text == "hi"));
        assert!(matches!(rx.recv().await, Some(HubMessage::Chat(m)) if m.comments[0].text == "yo"));
    }

    #[test]
    async fn directed_send_to_unknown_target_is_ok_noop() {
        let users = Connections::new();
        assert!(users.send_to_connection(Uuid::new_v4(), chat("x")).await.is_ok());
        assert!(users.send_to_user(Uuid::new_v4(), chat("x")).await.is_ok());
    }

    #[test]
    async fn full_channel_reports_dead_connection() {
        let users = Connections::new();
        let (tx, _rx) = mpsc::channel(1);
        let conn_id = Uuid::new_v4();
        users
            .register_connection(Uuid::new_v4(), conn_id, key(), tx, vec![TopicKey::Chat])
            .await;

        assert!(users.send_to_connection(conn_id, chat("1")).await.is_ok()); // fills capacity 1
        assert_eq!(users.send_to_connection(conn_id, chat("2")).await, Err(conn_id));
        // full → dead
    }

    #[test]
    async fn closed_channel_reports_dead_connection() {
        let users = Connections::new();
        let (tx, rx) = mpsc::channel(1);
        let conn_id = Uuid::new_v4();
        users
            .register_connection(Uuid::new_v4(), conn_id, key(), tx, vec![TopicKey::Chat])
            .await;
        drop(rx);
        assert_eq!(users.send_to_connection(conn_id, chat("1")).await, Err(conn_id));
    }

    #[test]
    async fn broadcast_clones_to_matching_topic_only() {
        let users = Connections::new();
        let (tx_chat, mut rx_chat) = mpsc::channel(4);
        let (tx_hub, mut rx_hub) = mpsc::channel(4);
        let chat_conn = Uuid::new_v4();
        let hub_conn = Uuid::new_v4();
        users
            .register_connection(Uuid::new_v4(), chat_conn, key(), tx_chat, vec![TopicKey::Chat])
            .await;
        users
            .register_connection(Uuid::new_v4(), hub_conn, key(), tx_hub, vec![TopicKey::Hub])
            .await;

        assert!(users.broadcast(chat("broadcast")).await.is_ok());
        assert!(matches!(rx_chat.recv().await, Some(HubMessage::Chat(_))));
        // Hub-topic connection must not receive a Chat broadcast.
        assert!(rx_hub.try_recv().is_err());
    }

    #[test]
    async fn remove_connection_cas_ignores_stale_id() {
        let users = Connections::new();
        let (tx, _rx) = mpsc::channel(1);
        let (user_id, conn_id) = (Uuid::new_v4(), Uuid::new_v4());
        users.register_connection(user_id, conn_id, key(), tx, vec![]).await;

        assert_eq!(users.remove_connection(user_id, Some(Uuid::new_v4())).await, None); // stale id → no-op
        assert_eq!(users.remove_connection(user_id, Some(conn_id)).await, Some(conn_id));
        assert_eq!(users.len().await, 0);
    }
}
