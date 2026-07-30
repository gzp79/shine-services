use crate::models::messages::{HubMessage, ToTopic, TopicKey};
use shine_infra::session::SessionKey;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
struct ConnectedUser {
    pub connection_id: Uuid,
    pub session_key: SessionKey,
}

struct Subscriber {
    topics: Vec<TopicKey>,
    tx: mpsc::UnboundedSender<HubMessage>,
}

/// Owns the hub's connection state: which users are connected (with their
/// session key, for the session checker) and which processes subscribe to
/// which topics. Mutated only from inside HubService's command loop.
#[derive(Clone)]
pub struct ConnectedUsers {
    sessions: Arc<RwLock<HashMap<Uuid, ConnectedUser>>>,
    subscribers: Arc<RwLock<Vec<Subscriber>>>,
}

impl ConnectedUsers {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Connects a user with the given connection record.
    /// If the user was already connected, this will overwrite the prior record.
    pub async fn connect(&self, user_id: Uuid, connection_id: Uuid, session_key: SessionKey) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(user_id, ConnectedUser { connection_id, session_key });
    }

    /// Removes the user's connection and returns the removed connection id.
    ///
    /// When `connection_id` is `Some`, the entry is removed only if it still matches the
    /// currently active connection for the user; a stale request whose connection has already
    /// been replaced by a fresh reconnect is ignored (returns `None`). `None` forces removal
    /// regardless of which connection is active.
    pub async fn disconnect(&self, user_id: Uuid, connection_id: Option<Uuid>) -> Option<Uuid> {
        let mut sessions = self.sessions.write().await;

        if let Some(expected) = connection_id {
            if sessions.get(&user_id).map(|connection| connection.connection_id) != Some(expected) {
                return None;
            }
        }

        sessions.remove(&user_id).map(|connection| connection.connection_id)
    }

    /// Returns the current local connection/session tuple for a user, when present.
    pub async fn find_connection(&self, user_id: Uuid) -> Option<(Uuid, SessionKey)> {
        let sessions = self.sessions.read().await;
        sessions
            .get(&user_id)
            .map(|connection| (connection.connection_id, connection.session_key))
    }

    pub async fn subscribe(&self, topics: Vec<TopicKey>, tx: mpsc::UnboundedSender<HubMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(Subscriber { topics, tx });
    }

    /// Delivers to every subscriber whose topic set includes this message's topic. The channel is
    /// unbounded, so a message is guaranteed delivered once accepted; the only failure is a closed
    /// receiver, in which case the subscriber is pruned.
    pub async fn publish(&self, message: HubMessage) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.retain(|subscriber| {
            if !subscriber.topics.contains(&message.topic()) {
                return true;
            }
            match subscriber.tx.send(message.clone()) {
                Ok(()) => true,
                Err(_) => {
                    log::error!("Subscriber closed, pruning {:?} subscriber", message.topic());
                    false
                }
            }
        });
    }
}

impl Default for ConnectedUsers {
    fn default() -> Self {
        Self::new()
    }
}
