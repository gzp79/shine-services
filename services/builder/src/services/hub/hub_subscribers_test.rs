//! Unit tests of [`Subscribers`]: topic filter, lossless delivery, release.

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
        }].into(),
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
