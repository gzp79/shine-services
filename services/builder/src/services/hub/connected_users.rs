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

/// Sending end of a subscription. Internal consumers get an unbounded, lossless channel; slow
/// consumers (e.g. remote clients) get a bounded one that is dropped rather than buffered without
/// limit when the consumer cannot keep up. See [`ConnectedUsers::publish`].
enum SubscriberTx {
    Unbounded(mpsc::UnboundedSender<HubMessage>),
    Bounded(mpsc::Sender<HubMessage>),
}

struct Subscriber {
    topics: Vec<TopicKey>,
    tx: SubscriberTx,
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

    /// Registers a lossless, unbounded subscriber. Intended for internal consumers (heartbeat,
    /// session checker) where dropping a lifecycle event would corrupt their connection tracker.
    pub async fn subscribe(&self, topics: Vec<TopicKey>, tx: mpsc::UnboundedSender<HubMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(Subscriber {
            topics,
            tx: SubscriberTx::Unbounded(tx),
        });
    }

    /// Registers a bounded subscriber. Intended for slow consumers (e.g. remote clients): if the
    /// consumer cannot keep up and its channel fills, `publish` drops the subscriber instead of
    /// buffering without limit, which closes the receiver.
    pub async fn subscribe_bounded(&self, topics: Vec<TopicKey>, tx: mpsc::Sender<HubMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(Subscriber {
            topics,
            tx: SubscriberTx::Bounded(tx),
        });
    }

    /// Delivers to every subscriber whose topic set includes this message's topic.
    ///
    /// Unbounded subscribers only fail on a closed receiver. Bounded subscribers additionally fail
    /// when the channel is full — a consumer too slow to drain the broadcast — in which case the
    /// subscriber is dropped (closing its receiver) rather than letting the hub buffer without
    /// limit.
    pub async fn publish(&self, message: HubMessage) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.retain(|subscriber| {
            if !subscriber.topics.contains(&message.topic()) {
                return true;
            }
            match &subscriber.tx {
                SubscriberTx::Unbounded(tx) => match tx.send(message.clone()) {
                    Ok(()) => true,
                    Err(_) => {
                        log::error!("Subscriber closed, pruning {:?} subscriber", message.topic());
                        false
                    }
                },
                SubscriberTx::Bounded(tx) => match tx.try_send(message.clone()) {
                    Ok(()) => true,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        log::warn!("Bounded subscriber lagging, dropping {:?} subscriber", message.topic());
                        false
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        log::error!("Subscriber closed, pruning {:?} subscriber", message.topic());
                        false
                    }
                },
            }
        });
    }

    /// Number of connected users and current subscribers, for status reporting.
    pub async fn stats(&self) -> (usize, usize) {
        let connections = self.sessions.read().await.len();
        let subscribers = self.subscribers.read().await.len();
        (connections, subscribers)
    }
}

impl Default for ConnectedUsers {
    fn default() -> Self {
        Self::new()
    }
}
