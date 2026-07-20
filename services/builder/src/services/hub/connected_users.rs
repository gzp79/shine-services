use crate::models::messages::{HubMessage, ToTopic, TopicKey};
use shine_infra::session::SessionKey;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
struct ConnectedUser {
    pub connection_id: Uuid,
    pub session_key: SessionKey,
}

struct Subscriber {
    topics: Vec<TopicKey>,
    tx: mpsc::Sender<HubMessage>,
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

    /// Removes the user if present and the session key matches.
    pub async fn disconnect(&self, user_id: Uuid, session_key: SessionKey) -> Option<Uuid> {
        let mut sessions = self.sessions.write().await;
        let should_remove = matches!(sessions.get(&user_id), Some(current) if current.session_key == session_key);
        if should_remove {
            sessions.remove(&user_id).map(|connection| connection.connection_id)
        } else {
            None
        }
    }

    pub async fn subscribe(&self, topics: Vec<TopicKey>, tx: mpsc::Sender<HubMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(Subscriber { topics, tx });
    }

    /// Delivers to every subscriber whose topic set includes this message's
    /// topic. A closed subscriber channel is logged and pruned. A full channel
    /// is a transient back-pressure signal: the message is dropped for that
    /// subscriber but the subscriber itself is kept.
    pub async fn publish(&self, message: HubMessage) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.retain(|subscriber| {
            if !subscriber.topics.contains(&message.topic()) {
                return true;
            }
            match subscriber.tx.try_send(message.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => {
                    log::warn!("Subscriber buffer full, dropping {:?} message", message.topic());
                    true
                }
                Err(TrySendError::Closed(_)) => {
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
