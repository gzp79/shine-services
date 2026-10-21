//! Unit tests of [`Subscribers`]: topic filter, lossless delivery, release.

use super::*;
use crate::models::messages::StreamMessage;
use shine_infra::db::command_log::StreamId;
use shine_test::test;
use tokio::sync::mpsc;

/// A message of the Stream topic; its text is the stream id.
fn payload(text: &str) -> HubMessage {
    HubMessage::Stream(StreamMessage::Reset {
        stream_id: StreamId::parse(text).unwrap(),
    })
}

#[test]
async fn publish_delivers_to_matching_topic_only() {
    let subscribers = Subscribers::new();
    let (tx_chat, mut rx_chat) = mpsc::unbounded_channel();
    let (tx_hub, mut rx_hub) = mpsc::unbounded_channel();
    subscribers.subscribe(vec![TopicKey::Stream], tx_chat).await;
    subscribers.subscribe(vec![TopicKey::Hub], tx_hub).await;

    subscribers.publish(payload("hi")).await;

    assert!(
        matches!(rx_chat.recv().await, Some(HubMessage::Stream(StreamMessage::Reset { stream_id })) if stream_id.as_str() == "hi")
    );
    // Hub-topic subscriber must not receive a Stream message.
    assert!(rx_hub.try_recv().is_err());
}

#[test]
async fn publish_prunes_closed_subscriber() {
    let subscribers = Subscribers::new();
    let (tx, rx) = mpsc::unbounded_channel();
    subscribers.subscribe(vec![TopicKey::Stream], tx).await;
    assert_eq!(subscribers.len().await, 1);

    drop(rx);
    subscribers.publish(payload("gone")).await;
    assert_eq!(subscribers.len().await, 0, "a closed subscriber is pruned on publish");
}
