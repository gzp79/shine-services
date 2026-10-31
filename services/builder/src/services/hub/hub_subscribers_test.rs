//! Unit tests of [`Subscribers`]: topic filter, lossless delivery, release.

use super::*;
use crate::services::hub_test_utils::ping;
use shine_test::test;
use tokio::sync::mpsc;

#[test]
async fn publish_delivers_to_matching_topic_only() {
    let subscribers = Subscribers::new();
    let (tx_chat, mut rx_chat) = mpsc::unbounded_channel();
    let (tx_hub, mut rx_hub) = mpsc::unbounded_channel();
    subscribers.subscribe(vec![TopicKey::Ping], tx_chat).await;
    subscribers.subscribe(vec![TopicKey::Hub], tx_hub).await;

    subscribers.publish(ping("hi")).await;

    assert!(matches!(rx_chat.recv().await, Some(HubMessage::Ping(m)) if m.id == "hi"));
    // Hub-topic subscriber must not receive a Chat message.
    assert!(rx_hub.try_recv().is_err());
}

#[test]
async fn publish_prunes_closed_subscriber() {
    let subscribers = Subscribers::new();
    let (tx, rx) = mpsc::unbounded_channel();
    subscribers.subscribe(vec![TopicKey::Ping], tx).await;
    assert_eq!(subscribers.len().await, 1);

    drop(rx);
    subscribers.publish(ping("gone")).await;
    assert_eq!(subscribers.len().await, 0, "a closed subscriber is pruned on publish");
}
