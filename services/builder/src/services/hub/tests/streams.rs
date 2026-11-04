//! How the hub carries stream subscriptions: a subscribe is a command of the loop, so it is ordered with
//! the connection's lifecycle, and it reaches the internal subscribers only.

use crate::{
    models::messages::{CommandFailure, HubEvent, HubMessage, StreamEvent, StreamMessage, SubscribeFailure, TopicKey},
    services::command_stream::StreamEgress,
    testing::hub::{Egress, TestHub, TestHubOptions},
};
use shine_game::replica::{StreamId, StreamKind};
use shine_test::test;
use std::sync::Arc;
use tokio::sync::mpsc::{self, error::TryRecvError};
use uuid::Uuid;

fn new_stream_id(name: &str) -> StreamId {
    StreamId::new(StreamKind::Chat, name).unwrap()
}

/// Describes messages currently queued on a bus subscription.
fn drain_messages(rx: &mut mpsc::UnboundedReceiver<HubMessage>) -> Vec<String> {
    let mut messages = Vec::new();
    while let Ok(message) = rx.try_recv() {
        messages.push(match message {
            HubMessage::Hub(HubEvent::UserConnected { .. }) => "connected".to_string(),
            HubMessage::Hub(HubEvent::UserDisconnected { .. }) => "disconnected".to_string(),
            HubMessage::StreamEvent(StreamEvent::Subscribed { stream_id, .. }) => format!("subscribed:{stream_id}"),
            HubMessage::StreamEvent(StreamEvent::Unsubscribed { stream_id, .. }) => format!("unsubscribed:{stream_id}"),
            other => format!("{other:?}"),
        });
    }
    messages
}

#[test(serial = "hub-service")]
async fn a_subscribe_reaches_the_internal_subscribers_and_never_the_sockets() {
    let mut test = TestHub::default_from_env().await.unwrap();
    let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;
    let (connection, mut socket) = test.connect(Uuid::new_v4(), 8, vec![TopicKey::Hub, TopicKey::Ping], None);

    test.hub
        .request_stream_subscribe(connection, new_stream_id("s1"))
        .unwrap();
    test.hub
        .request_stream_unsubscribe(connection, new_stream_id("s1"))
        .unwrap();
    test.wait_all_commands().await;

    assert_eq!(
        drain_messages(&mut internal),
        vec!["subscribed:chat:s1".to_string(), "unsubscribed:chat:s1".to_string()]
    );
    // The socket got the lifecycle event of its own connection, and nothing about the streams.
    while let Ok(message) = socket.try_recv() {
        assert!(matches!(message, HubMessage::Hub(_)), "{message:?}");
    }
}

#[test(serial = "hub-service")]
async fn a_subscribe_of_a_connection_that_is_not_registered_is_ignored() {
    let mut test = TestHub::default_from_env().await.unwrap();
    let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;

    test.hub
        .request_stream_subscribe(Uuid::new_v4(), new_stream_id("s1"))
        .unwrap();
    test.hub
        .request_stream_unsubscribe(Uuid::new_v4(), new_stream_id("s1"))
        .unwrap();
    test.wait_all_commands().await;

    assert!(drain_messages(&mut internal).is_empty());
}

#[test(serial = "hub-service")]
async fn a_subscribe_is_ordered_with_the_lifecycle_of_its_connection() {
    let mut test = TestHub::default_from_env().await.unwrap();
    let user = Uuid::new_v4();
    let mut internal = test.hub.subscribe(vec![TopicKey::Hub, TopicKey::Stream]).await;
    let (connection, _socket) = test.connect_ping(user, 8);

    test.hub
        .request_stream_subscribe(connection, new_stream_id("s1"))
        .unwrap();
    test.hub.request_disconnection(user, connection).unwrap();
    // Too late: the connection is gone.
    test.hub
        .request_stream_subscribe(connection, new_stream_id("s2"))
        .unwrap();
    test.wait_all_commands().await;

    // The sentinel connection of the barrier follows; what matters is what comes first.
    let seen = drain_messages(&mut internal);
    assert_eq!(
        seen[..3],
        ["connected", "subscribed:chat:s1", "disconnected"],
        "{seen:?}"
    );
    assert!(!seen.iter().any(|m| m == "subscribed:chat:s2"), "{seen:?}");
}

#[test(serial = "hub-service")]
async fn the_hub_sends_stream_messages_to_a_connection_by_its_id() {
    let mut test = TestHub::default_from_env().await.unwrap();
    let (connection, mut socket) = test.connect_ping(Uuid::new_v4(), 8);
    test.wait_all_commands().await;
    let message = Arc::new(StreamMessage::Reset { stream_id: new_stream_id("s1") });

    let queued = test.hub.send(connection, message.clone()).await;
    let unknown = test.hub.send(Uuid::new_v4(), message.clone()).await;

    assert!(queued);
    assert!(!unknown);
    assert!(matches!(
        socket.try_recv(),
        Ok(HubMessage::StreamMessage(sent)) if *sent == *message
    ));
    assert!(matches!(socket.try_recv(), Err(TryRecvError::Empty)));
}

// ---------------------------------------------------------------------------------------------
// With the stream workers behind it
// ---------------------------------------------------------------------------------------------

mod served {
    use super::*;
    use crate::services::command_stream::StreamConfig;
    use shine_game::replica::{encode_command, ChatCommand, ChatState, PixelMapCommand, PixelMapState};
    use shine_test::test;
    use std::time::Duration;
    use tokio::time::timeout;

    async fn create_test_hub() -> TestHub {
        let stream_config = StreamConfig {
            unload_grace: Duration::from_millis(20),
            ..StreamConfig::quiet()
        };
        TestHub::from_env(TestHubOptions {
            with_streams: Some(stream_config),
            ..TestHubOptions::default()
        })
        .await
        .unwrap()
    }

    /// The next stream message the connection gets.
    async fn next_stream_message(socket: &mut Egress) -> Arc<StreamMessage> {
        timeout(Duration::from_secs(5), async {
            loop {
                match socket.recv().await {
                    Some(HubMessage::StreamMessage(message)) => return message,
                    Some(_) => {}
                    None => panic!("the connection was closed"),
                }
            }
        })
        .await
        .expect("a stream message")
    }

    fn chat_command(text: &str) -> String {
        encode_command::<ChatState>(&ChatCommand::Add { text: text.into() }).unwrap()
    }

    #[test(serial = "hub-service")]
    async fn a_connection_subscribes_through_the_hub_and_gets_the_commands_of_two_kinds() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let (chat, map) = (
            StreamId::new(StreamKind::Chat, format!("chat-{}", Uuid::new_v4().simple())).unwrap(),
            StreamId::new(StreamKind::PixelMap, format!("map-{}", Uuid::new_v4().simple())).unwrap(),
        );
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;

        test.stream_subscribe(connection, user, &chat).await.unwrap();
        test.stream_subscribe(connection, user, &map).await.unwrap();
        let mut synced = [
            next_stream_message(&mut socket).await,
            next_stream_message(&mut socket).await,
        ];
        let paint_data = encode_command::<PixelMapState>(&PixelMapCommand::Paint { x: 3, y: 4, color: 5 }).unwrap();
        test.hub
            .send_stream_command(
                connection,
                Uuid::new_v4(),
                &chat,
                Uuid::new_v4(),
                &chat_command("hello"),
            )
            .await
            .unwrap();
        test.hub
            .send_stream_command(connection, Uuid::new_v4(), &map, Uuid::new_v4(), &paint_data)
            .await
            .unwrap();
        let mut commands = [
            next_stream_message(&mut socket).await,
            next_stream_message(&mut socket).await,
        ];

        synced.sort_by_key(|m| m.stream_id().to_string());
        commands.sort_by_key(|m| m.stream_id().to_string());
        let mut expected = [chat.to_string(), map.to_string()];
        expected.sort();
        assert_eq!(
            synced.iter().map(|m| m.stream_id().to_string()).collect::<Vec<_>>(),
            expected
        );
        assert!(synced
            .iter()
            .all(|m| matches!(m.as_ref(), StreamMessage::Sync { commands, .. } if commands.is_empty())));
        assert!(commands.iter().all(|m| matches!(m.as_ref(), StreamMessage::Commands { commands, .. } if commands.len() == 1 && commands[0].version == 1)));
    }

    #[test(serial = "hub-service")]
    async fn a_disconnect_releases_the_workers_of_the_connection() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let id = StreamId::new(StreamKind::Chat, format!("room-{}", Uuid::new_v4().simple())).unwrap();
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        test.stream_subscribe(connection, user, &id).await.unwrap();
        next_stream_message(&mut socket).await;
        let streams = test.stream_registry.unwrap().clone();
        assert_eq!(streams.worker_count(), 1);

        test.hub.request_disconnection(user, connection).unwrap();

        timeout(Duration::from_secs(5), async {
            while streams.worker_count() != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the worker ended");
    }

    #[test(serial = "hub-service")]
    async fn a_subscribe_or_a_command_is_refused_with_its_reason() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let (connection, _socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        let hub = &test.hub;
        let id = StreamId::new(StreamKind::PixelMap, format!("canvas-{}", Uuid::new_v4().simple())).unwrap();
        let off_the_map = encode_command::<PixelMapState>(&PixelMapCommand::Paint { x: 99, y: 0, color: 1 }).unwrap();
        let send = |stream: &StreamId, command: String| {
            let hub = hub.clone();
            let stream = stream.clone();
            async move {
                hub.send_stream_command(connection, user, &stream, Uuid::new_v4(), &command)
                    .await
            }
        };

        // Before the subscribe.
        assert_eq!(
            send(&id, chat_command("early")).await,
            Err(CommandFailure::NotSubscribed)
        );
        test.stream_subscribe(connection, user, &id).await.unwrap();
        // The kind in the stream identity decides what is valid.
        assert_eq!(send(&id, off_the_map).await, Err(CommandFailure::Invalid));
        assert_eq!(
            send(&id, chat_command("not a paint")).await,
            Err(CommandFailure::Invalid)
        );
        // After the unsubscribe, with no waiting for the loop in between.
        hub.unsubscribe_stream(connection, id.clone()).unwrap();
        assert_eq!(
            send(&id, chat_command("late")).await,
            Err(CommandFailure::NotSubscribed)
        );
    }

    #[test(serial = "hub-service")]
    async fn a_connection_that_is_gone_is_not_subscribed_to_anything() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let id = StreamId::new(StreamKind::Chat, format!("room-{}", Uuid::new_v4().simple())).unwrap();
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        test.stream_subscribe(connection, user, &id).await.unwrap();
        next_stream_message(&mut socket).await;

        test.hub.request_disconnection(user, connection).unwrap();
        test.wait_all_commands().await;

        let result = test
            .hub
            .send_stream_command(connection, user, &id, Uuid::new_v4(), &chat_command("ghost"))
            .await;
        assert_eq!(result, Err(CommandFailure::NotSubscribed));
    }
}

#[test(serial = "hub-service")]
async fn a_hub_without_streams_refuses_them() {
    let test = TestHub::default_from_env().await.unwrap();
    let (connection, _socket) = test.connect_ping(Uuid::new_v4(), 8);
    let id = new_stream_id("s1");

    assert_eq!(
        test.hub.subscribe_stream(connection, Uuid::new_v4(), id.clone()).await,
        Err(SubscribeFailure::Failed)
    );
    assert_eq!(
        test.hub
            .send_stream_command(connection, Uuid::new_v4(), &id, Uuid::new_v4(), "{}")
            .await,
        Err(CommandFailure::NotSubscribed)
    );
}
