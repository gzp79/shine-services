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
                    log::error!("Subscriber closed, pruning {:?} subscriber", message.topic());
                    false
                }
            }
        });
    }

    /// Number of current internal subscribers, for status reporting.
    pub async fn len(&self) -> usize {
        self.subscribers.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::messages::{ChatBatch, ChatComment};
    use shine_test::test;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    fn chat(text: &str) -> HubMessage {
        HubMessage::Chat(ChatBatch {
            comments: vec![ChatComment {
                id: "0-0".to_string(),
                user_id: Uuid::new_v4(),
                text: text.to_string(),
            }],
        })
    }

    #[test]
    async fn publish_delivers_to_matching_topic_only() {
        let subscribers = Subscribers::new();
        let (tx_chat, mut rx_chat) = mpsc::unbounded_channel();
        let (tx_hub, mut rx_hub) = mpsc::unbounded_channel();
        subscribers.subscribe(vec![TopicKey::Chat], tx_chat).await;
        subscribers.subscribe(vec![TopicKey::Hub], tx_hub).await;

        subscribers.publish(chat("hi")).await;

        assert!(matches!(rx_chat.recv().await, Some(HubMessage::Chat(m)) if m.comments[0].text == "hi"));
        // Hub-topic subscriber must not receive a Chat message.
        assert!(rx_hub.try_recv().is_err());
    }

    #[test]
    async fn publish_prunes_closed_subscriber() {
        let subscribers = Subscribers::new();
        let (tx, rx) = mpsc::unbounded_channel();
        subscribers.subscribe(vec![TopicKey::Chat], tx).await;
        assert_eq!(subscribers.len().await, 1);

        drop(rx);
        subscribers.publish(chat("gone")).await;
        assert_eq!(subscribers.len().await, 0, "a closed subscriber is pruned on publish");
    }
}
