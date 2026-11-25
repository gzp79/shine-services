//! Unit tests of [`Subscribers`]: topic filter, lossless delivery, release.

use super::*;
use crate::testing::TestMessage;
use shine_test::test;

#[test]
async fn publish_delivers_to_matching_topic_only() {
    let subscribers = Subscribers::new();
    let mut rx_chat = subscribers.subscribe(vec![TopicKey::Ping]).await;
    let mut rx_hub = subscribers.subscribe(vec![TopicKey::Hub]).await;

    subscribers.publish(TestMessage::new_ping("hi")).await;

    assert!(matches!(rx_chat.recv().await, Some(HubMessage::Ping(m)) if m.id == "hi"));
    // Hub-topic subscriber must not receive a Chat message.
    assert!(rx_hub.try_recv().is_err());
}

#[test]
async fn publish_prunes_closed_subscriber() {
    let subscribers = Subscribers::new();
    let rx = subscribers.subscribe(vec![TopicKey::Ping]).await;
    assert_eq!(subscribers.subscription_count().await, 1);

    drop(rx);
    subscribers.publish(TestMessage::new_ping("gone")).await;
    assert_eq!(subscribers.subscription_count().await, 0, "a closed subscriber is pruned on publish");
}
