use crate::models::messages::{HubMessage, ToTopic, TopicKey};
use tokio::sync::{mpsc, RwLock};

struct Subscriber {
    topics: Vec<TopicKey>,
    tx: mpsc::UnboundedSender<HubMessage>,
}

/// The internal, lossless fan-out subscribers (heartbeat, session checker). Each receives every
/// message whose topic it subscribed to.
#[derive(Default)]
pub struct Subscribers {
    subscribers: RwLock<Vec<Subscriber>>,
}

impl Subscribers {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn subscribe(&self, topics: Vec<TopicKey>, tx: mpsc::UnboundedSender<HubMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(Subscriber { topics, tx });
    }

    /// Delivers to every subscriber whose topic set includes this message's topic, pruning any
    /// whose receiver has closed.
    pub async fn publish(&self, message: HubMessage) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.retain(|subscriber| {
            if !subscriber.topics.contains(&message.topic()) {
                return true;
            }
            match subscriber.tx.send(message.clone()) {
                Ok(()) => true,
                Err(_) => {
                    tracing::error!("Subscriber closed, pruning {:?} subscriber", message.topic());
                    false
                }
            }
        });
    }

    /// Removes every subscriber, closing their channels.
    pub async fn clear(&self) {
        self.subscribers.write().await.clear();
    }

    /// Number of current internal subscribers, for status reporting.
    pub async fn len(&self) -> usize {
        self.subscribers.read().await.len()
    }
}

#[cfg(test)]
#[path = "hub_subscribers_test.rs"]
mod hub_subscribers_test;
