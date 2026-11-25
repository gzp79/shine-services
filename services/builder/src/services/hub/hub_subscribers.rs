use crate::models::messages::{HubMessage, ToTopic, TopicKey};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

struct Subscriber {
    topics: Vec<TopicKey>,
    tx: mpsc::UnboundedSender<HubMessage>,
}

/// The internal, lossless fan-out subscribers (heartbeat, session checker). Each receives every
/// message whose topic it subscribed to.
#[derive(Clone, Default)]
pub struct Subscribers {
    subscribers: Arc<RwLock<Vec<Subscriber>>>,
}

impl Subscribers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribes to the topics on an unbounded, lossless channel.
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> mpsc::UnboundedReceiver<HubMessage> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.subscribers.write().await.push(Subscriber { topics, tx });
        rx
    }

    pub async fn subscription_count(&self) -> usize {
        self.subscribers.read().await.len()
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
}

#[cfg(test)]
#[path = "hub_subscribers_test.rs"]
mod hub_subscribers_test;
