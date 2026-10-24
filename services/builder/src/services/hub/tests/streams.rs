//! How the hub carries stream subscriptions: a subscribe is a command of the loop, so it is ordered with
//! the connection's lifecycle, and it reaches the internal subscribers only.

use crate::{
    models::messages::{HubEvent, HubMessage, NackReason, StreamEvent, StreamMessage, SubscribeFailure, TopicKey},
    repositories::command_log::StreamId,
    services::{
        hub_test_utils::{barrier, connect, connect_on, create_test_hub, TestHub, TestHubOptions},
        StreamEgress,
    },
};
use shine_test::test;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;
use uuid::Uuid;

async fn hub() -> Option<TestHub> {
    create_test_hub(TestHubOptions::default()).await
}

fn stream(name: &str) -> StreamId {
    StreamId::parse(name).unwrap()
}

/// What is queued on a bus subscription right now.
fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<HubMessage>) -> Vec<HubMessage> {
    let mut messages = Vec::new();
    while let Ok(message) = rx.try_recv() {
        messages.push(message);
    }
    messages
}

fn describe(messages: &[HubMessage]) -> Vec<String> {
    messages
        .iter()
        .map(|message| match message {
            HubMessage::Hub(HubEvent::UserConnected { .. }) => "connected".to_string(),
            HubMessage::Hub(HubEvent::UserDisconnected { .. }) => "disconnected".to_string(),
            HubMessage::StreamEvent(StreamEvent::Subscribed { stream_id, kind, .. }) => {
                format!("subscribed:{stream_id}:{kind}")
            }
            HubMessage::StreamEvent(StreamEvent::Unsubscribed { stream_id, .. }) => format!("unsubscribed:{stream_id}"),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test(serial = "hub-service")]
async fn a_subscribe_reaches_the_internal_subscribers_and_never_the_sockets() {
    let Some(mut test) = hub().await else { return };
    let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;
    let (connection, mut socket) = connect_on(&test.hub, Uuid::new_v4(), 8, vec![TopicKey::Hub, TopicKey::Chat]);

    test.hub
        .request_stream_subscribe(connection, stream("s1"), "chat".to_string())
        .unwrap();
    test.hub.request_stream_unsubscribe(connection, stream("s1")).unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        describe(&drain(&mut internal)),
        vec!["subscribed:s1:chat".to_string(), "unsubscribed:s1".to_string()]
    );
    // The socket got the lifecycle event of its own connection, and nothing about the streams.
    while let Ok(message) = socket.try_recv() {
        assert!(matches!(message, HubMessage::Hub(_)), "{message:?}");
    }
}

#[test(serial = "hub-service")]
async fn a_subscribe_of_a_connection_that_is_not_registered_is_ignored() {
    let Some(mut test) = hub().await else { return };
    let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;

    test.hub
        .request_stream_subscribe(Uuid::new_v4(), stream("s1"), "chat".to_string())
        .unwrap();
    test.hub
        .request_stream_unsubscribe(Uuid::new_v4(), stream("s1"))
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert!(drain(&mut internal).is_empty());
}

#[test(serial = "hub-service")]
async fn a_subscribe_is_ordered_with_the_lifecycle_of_its_connection() {
    let Some(mut test) = hub().await else { return };
    let user = Uuid::new_v4();
    let mut internal = test.hub.subscribe(vec![TopicKey::Hub, TopicKey::Stream]).await;
    let (connection, _socket) = connect(&test.hub, user, 8);

    test.hub
        .request_stream_subscribe(connection, stream("s1"), "chat".to_string())
        .unwrap();
    test.hub.request_disconnection(user, connection).unwrap();
    // Too late: the connection is gone.
    test.hub
        .request_stream_subscribe(connection, stream("s2"), "chat".to_string())
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    // The sentinel connection of the barrier follows; what matters is what comes first.
    let seen = describe(&drain(&mut internal));
    assert_eq!(
        seen[..3],
        ["connected", "subscribed:s1:chat", "disconnected"],
        "{seen:?}"
    );
    assert!(!seen.iter().any(|m| m == "subscribed:s2:chat"), "{seen:?}");
}

#[test(serial = "hub-service")]
async fn the_hub_sends_stream_messages_to_a_connection_by_its_id() {
    let Some(mut test) = hub().await else { return };
    let (connection, mut socket) = connect(&test.hub, Uuid::new_v4(), 8);
    barrier(&test.hub, &mut test.events).await;
    let message = Arc::new(StreamMessage::Reset { stream_id: stream("s1") });

    let queued = test.hub.send(connection, message.clone()).await;
    let unknown = test.hub.send(Uuid::new_v4(), message.clone()).await;

    assert!(queued);
    assert!(!unknown);
    assert!(matches!(
        socket.try_recv(),
        Ok(HubMessage::Stream(sent)) if *sent == *message
    ));
    assert!(matches!(socket.try_recv(), Err(TryRecvError::Empty)));
}

// ---------------------------------------------------------------------------------------------
// With the stream workers behind it
// ---------------------------------------------------------------------------------------------

mod served {
    use super::*;
    use crate::{
        repositories::{
            command_inbox::tests::test_utils::{create_test_inbox, TestInbox},
            command_log::tests::test_utils::{create_test_log, TestLog},
        },
        services::{command_stream::WorkerConfig, hub_test_utils::Egress},
    };
    use shine_game::replica::{encode_command, ChatCommand, ChatState, PixelMapCommand, PixelMapState, ReplayState};
    use shine_test::test;
    use std::time::Duration;
    use tokio::time::timeout;

    struct Served {
        test: TestHub,
        _inbox: TestInbox,
        _log: TestLog,
    }

    async fn served() -> Option<Served> {
        let log = create_test_log().await?;
        let inbox = create_test_inbox(60).await?;
        let worker = WorkerConfig {
            backstop: Duration::from_secs(3600),
            min_wake_spacing: Duration::ZERO,
            unload_grace: Duration::from_millis(200),
            max_failures: 3,
        };
        let test = create_test_hub(TestHubOptions {
            streams: Some((log.db.clone(), inbox.db.clone(), worker)),
            ..TestHubOptions::default()
        })
        .await?;
        test.hub.streams().expect("the hub has streams").listen().await.unwrap();
        Some(Served { test, _inbox: inbox, _log: log })
    }

    /// The next stream message the connection gets.
    async fn next_stream_message(socket: &mut Egress) -> Arc<StreamMessage> {
        timeout(Duration::from_secs(5), async {
            loop {
                match socket.recv().await {
                    Some(HubMessage::Stream(message)) => return message,
                    Some(_) => {}
                    None => panic!("the connection was closed"),
                }
            }
        })
        .await
        .expect("a stream message")
    }

    async fn subscribe(served: &Served, connection: Uuid, stream_id: &StreamId, kind: &str) {
        served
            .test
            .hub
            .subscribe_stream(connection, stream_id.clone(), kind)
            .await
            .unwrap();
    }

    fn chat_command(text: &str) -> String {
        encode_command::<ChatState>(&ChatCommand::Add { text: text.into() }).unwrap()
    }

    #[test(serial = "hub-service")]
    async fn a_connection_subscribes_through_the_hub_and_gets_the_commands_of_two_kinds() {
        let Some(mut served) = served().await else { return };
        let (chat, map) = (stream("chat-1"), stream("map-1"));
        let (chat, map) = (
            StreamId::parse(&format!("{chat}-{}", Uuid::new_v4().simple())).unwrap(),
            StreamId::parse(&format!("{map}-{}", Uuid::new_v4().simple())).unwrap(),
        );
        let (connection, mut socket) = connect(&served.test.hub, Uuid::new_v4(), 32);
        barrier(&served.test.hub, &mut served.test.events).await;

        subscribe(&served, connection, &chat, ChatState::KIND).await;
        subscribe(&served, connection, &map, PixelMapState::KIND).await;
        let mut synced = [
            next_stream_message(&mut socket).await,
            next_stream_message(&mut socket).await,
        ];
        let paint_data = encode_command::<PixelMapState>(&PixelMapCommand::Paint { x: 3, y: 4, color: 5 }).unwrap();
        let hub = &served.test.hub;
        hub.send_stream_command(
            connection,
            Uuid::new_v4(),
            &chat,
            Uuid::new_v4(),
            &chat_command("hello"),
        )
        .await
        .unwrap();
        hub.send_stream_command(connection, Uuid::new_v4(), &map, Uuid::new_v4(), &paint_data)
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
        let Some(mut served) = served().await else { return };
        let user = Uuid::new_v4();
        let id = StreamId::parse(&format!("room-{}", Uuid::new_v4().simple())).unwrap();
        let (connection, mut socket) = connect(&served.test.hub, user, 32);
        barrier(&served.test.hub, &mut served.test.events).await;
        subscribe(&served, connection, &id, ChatState::KIND).await;
        next_stream_message(&mut socket).await;
        let streams = served.test.hub.streams().unwrap().clone();
        assert_eq!(streams.worker_count(), 1);

        served.test.hub.request_disconnection(user, connection).unwrap();

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
        let Some(mut served) = served().await else { return };
        let user = Uuid::new_v4();
        let (connection, _socket) = connect(&served.test.hub, user, 32);
        barrier(&served.test.hub, &mut served.test.events).await;
        let hub = &served.test.hub;
        let id = StreamId::parse(&format!("canvas-{}", Uuid::new_v4().simple())).unwrap();
        let off_the_map = encode_command::<PixelMapState>(&PixelMapCommand::Paint { x: 99, y: 0, color: 1 }).unwrap();
        let send = |stream: &StreamId, command: String| {
            let hub = hub.clone();
            let stream = stream.clone();
            async move {
                hub.send_stream_command(connection, user, &stream, Uuid::new_v4(), &command)
                    .await
            }
        };

        // Before the subscribe, and with a kind nobody serves.
        assert_eq!(send(&id, chat_command("early")).await, Err(NackReason::NotSubscribed));
        assert_eq!(
            hub.subscribe_stream(connection, id.clone(), "nothing").await,
            Err(SubscribeFailure::UnknownKind)
        );
        subscribe(&served, connection, &id, PixelMapState::KIND).await;
        // The kind of the stream decides what is valid, and what it can be opened as.
        assert_eq!(send(&id, off_the_map).await, Err(NackReason::Invalid));
        assert_eq!(send(&id, chat_command("not a paint")).await, Err(NackReason::Invalid));
        assert_eq!(
            hub.subscribe_stream(connection, id.clone(), ChatState::KIND).await,
            Err(SubscribeFailure::KindMismatch)
        );
        // After the unsubscribe, with no waiting for the loop in between.
        hub.unsubscribe_stream(connection, id.clone()).unwrap();
        assert_eq!(send(&id, chat_command("late")).await, Err(NackReason::NotSubscribed));
    }

    #[test(serial = "hub-service")]
    async fn a_connection_that_is_gone_is_not_subscribed_to_anything() {
        let Some(mut served) = served().await else { return };
        let user = Uuid::new_v4();
        let id = StreamId::parse(&format!("room-{}", Uuid::new_v4().simple())).unwrap();
        let (connection, mut socket) = connect(&served.test.hub, user, 32);
        barrier(&served.test.hub, &mut served.test.events).await;
        subscribe(&served, connection, &id, ChatState::KIND).await;
        next_stream_message(&mut socket).await;

        served.test.hub.request_disconnection(user, connection).unwrap();
        barrier(&served.test.hub, &mut served.test.events).await;

        let result = served
            .test
            .hub
            .send_stream_command(connection, user, &id, Uuid::new_v4(), &chat_command("ghost"))
            .await;
        assert_eq!(result, Err(NackReason::NotSubscribed));
    }
}

#[test(serial = "hub-service")]
async fn a_hub_without_streams_refuses_them() {
    let Some(test) = hub().await else { return };
    let (connection, _socket) = connect(&test.hub, Uuid::new_v4(), 8);
    let id = stream("s1");

    assert_eq!(
        test.hub.subscribe_stream(connection, id.clone(), "chat").await,
        Err(SubscribeFailure::Failed)
    );
    assert_eq!(
        test.hub
            .send_stream_command(connection, Uuid::new_v4(), &id, Uuid::new_v4(), "{}")
            .await,
        Err(NackReason::NotSubscribed)
    );
}
