//! How the hub carries stream subscriptions: a subscribe is a command of the loop, so it is ordered with
//! the connection's lifecycle, and it reaches the internal subscribers only.

use crate::{
    models::messages::{
        CommandFailure, DisconnectReason, HubEvent, HubMessage, StreamEvent, StreamMessage, SubscribeFailure, TopicKey,
    },
    services::{command_stream::StreamEgress, hub::control_command::ControlCommand},
    testing::{
        command_stream::TestStreamId,
        hub::{TestEgress, TestHub, TestHubOptions},
        TestMessage,
    },
};
use shine_game::replica::StreamId;
use shine_test::test;
use std::sync::Arc;
use tokio::sync::mpsc::{self, error::TryRecvError};
use uuid::Uuid;

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
    let mut test = TestHub::default_from_env().await;
    let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;
    let (connection, mut socket) = test.connect(Uuid::new_v4(), 8, vec![TopicKey::Hub, TopicKey::Ping], None);

    test.hub
        .send_control(ControlCommand::SubscribeStream {
            connection_id: connection,
            stream_id: TestStreamId::new_chat_with("s1"),
        })
        .unwrap();
    test.hub
        .send_control(ControlCommand::UnsubscribeStream {
            connection_id: connection,
            stream_id: TestStreamId::new_chat_with("s1"),
        })
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
    let mut test = TestHub::default_from_env().await;
    let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;

    test.hub
        .send_control(ControlCommand::SubscribeStream {
            connection_id: Uuid::new_v4(),
            stream_id: TestStreamId::new_chat_with("s1"),
        })
        .unwrap();
    test.hub
        .send_control(ControlCommand::UnsubscribeStream {
            connection_id: Uuid::new_v4(),
            stream_id: TestStreamId::new_chat_with("s1"),
        })
        .unwrap();
    test.wait_all_commands().await;

    assert!(drain_messages(&mut internal).is_empty());
}

#[test(serial = "hub-service")]
async fn a_subscribe_is_ordered_with_the_lifecycle_of_its_connection() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();
    let mut internal = test.hub.subscribe(vec![TopicKey::Hub, TopicKey::Stream]).await;
    let (connection, _socket) = test.connect_ping(user, 8);

    test.hub
        .send_control(ControlCommand::SubscribeStream {
            connection_id: connection,
            stream_id: TestStreamId::new_chat_with("s1"),
        })
        .unwrap();
    test.hub
        .request_disconnect(user, connection, DisconnectReason::Client)
        .unwrap();
    // Too late: the connection is gone.
    test.hub
        .send_control(ControlCommand::SubscribeStream {
            connection_id: connection,
            stream_id: TestStreamId::new_chat_with("s2"),
        })
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
    let mut test = TestHub::default_from_env().await;
    let (connection, mut socket) = test.connect_ping(Uuid::new_v4(), 8);
    test.wait_all_commands().await;
    let message = Arc::new(StreamMessage::Checkpoint {
        stream_id: TestStreamId::new_chat_with("s1"),
        version: 1,
        state_hash: 0xAB,
    });

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
    use crate::services::command_stream::{InboxConfig, StreamAccessConfig, StreamConfig};
    use shine_test::test;
    use std::time::Duration;
    use tokio::time::{sleep, timeout};

    async fn create_test_hub() -> TestHub {
        let stream_config = StreamConfig {
            worker_unload_grace: Duration::from_millis(20),
            ..StreamConfig::test_default()
        };
        TestHub::from_env(TestHubOptions {
            with_streams: Some(stream_config),
            ..TestHubOptions::default()
        })
        .await
    }

    /// The next stream message the connection gets.
    async fn next_stream_message(socket: &mut TestEgress) -> Arc<StreamMessage> {
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

    #[test(serial = "hub-service")]
    async fn a_failed_stream_is_closed_for_all_subscribers_without_closing_their_connections() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let (failed, kept) = (TestStreamId::new_chat(), TestStreamId::new_chat());
        let (first, mut first_socket) = test.connect_ping(user, 32);
        let (second, mut second_socket) = test.connect_ping(other_user, 32);
        test.wait_all_commands().await;
        test.stream_subscribe(first, user, &failed).await.unwrap();
        next_stream_message(&mut first_socket).await;
        test.stream_subscribe(first, user, &kept).await.unwrap();
        next_stream_message(&mut first_socket).await;
        test.stream_subscribe(second, other_user, &failed).await.unwrap();
        next_stream_message(&mut second_socket).await;
        let mut internal = test.hub.subscribe(vec![TopicKey::Stream]).await;

        test.hub.notify_failed(&failed).await;
        test.wait_all_commands().await;

        for socket in [&mut first_socket, &mut second_socket] {
            assert_eq!(
                *next_stream_message(socket).await,
                StreamMessage::Desynced { stream_id: failed.clone() }
            );
            assert!(matches!(socket.try_recv(), Err(TryRecvError::Empty)));
        }
        assert_eq!(drain_messages(&mut internal), vec![format!("unsubscribed:{failed}"); 2]);

        let command_id = Uuid::new_v4();
        assert_eq!(
            test.hub
                .send_stream_command(
                    first,
                    user,
                    &failed,
                    command_id,
                    &TestMessage::new_chat_command("closed")
                )
                .await,
            Err(CommandFailure::NotSubscribed)
        );
        test.hub
            .send_stream_command(
                first,
                user,
                &kept,
                Uuid::new_v4(),
                &TestMessage::new_chat_command("kept"),
            )
            .await
            .unwrap();
        assert!(matches!(
            &*next_stream_message(&mut first_socket).await,
            StreamMessage::Commands { stream_id, .. } if *stream_id == kept
        ));
        test.stream_subscribe(second, other_user, &failed).await.unwrap();
        assert!(matches!(
            &*next_stream_message(&mut second_socket).await,
            StreamMessage::Sync { stream_id, .. } if *stream_id == failed
        ));
    }

    #[test(serial = "hub-service")]
    async fn a_connection_subscribes_through_the_hub_and_gets_the_commands_of_two_kinds() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let (chat, map) = (TestStreamId::new_chat(), TestStreamId::new_pixel_map());
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;

        test.stream_subscribe(connection, user, &chat).await.unwrap();
        test.stream_subscribe(connection, user, &map).await.unwrap();
        let mut synced = [
            next_stream_message(&mut socket).await,
            next_stream_message(&mut socket).await,
        ];
        let paint_data = TestMessage::new_paint_command(3, 4, 5);
        test.hub
            .send_stream_command(
                connection,
                Uuid::new_v4(),
                &chat,
                Uuid::new_v4(),
                &TestMessage::new_chat_command("hello"),
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
    async fn subscribing_again_syncs_the_connection_again_from_the_start() {
        let mut test = create_test_hub().await;
        let (user, id) = (Uuid::new_v4(), TestStreamId::new_chat());
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        let send = |text: &str| {
            let hub = test.hub.clone();
            let (id, command) = (id.clone(), TestMessage::new_chat_command(text));
            async move {
                hub.send_stream_command(connection, user, &id, Uuid::new_v4(), &command)
                    .await
            }
        };

        test.stream_subscribe(connection, user, &id).await.unwrap();
        assert!(
            matches!(&*next_stream_message(&mut socket).await, StreamMessage::Sync { commands, .. } if commands.is_empty())
        );
        send("first").await.unwrap();
        assert!(
            matches!(&*next_stream_message(&mut socket).await, StreamMessage::Commands { commands, .. } if commands.len() == 1)
        );

        // The second subscribe is a request for a new sync: the whole history again, then only what is new.
        test.stream_subscribe(connection, user, &id).await.unwrap();
        let message = next_stream_message(&mut socket).await;
        assert!(
            matches!(&*message, StreamMessage::Sync { commands, .. } if commands.len() == 1 && commands[0].version == 1),
            "{message:?}"
        );
        send("second").await.unwrap();
        let message = next_stream_message(&mut socket).await;
        assert!(
            matches!(&*message, StreamMessage::Commands { commands, .. } if commands.len() == 1 && commands[0].version == 2),
            "{message:?}"
        );
    }

    #[test(serial = "hub-service")]
    async fn a_command_id_is_the_clients_own_so_a_repeated_one_gets_a_version_each() {
        let mut test = create_test_hub().await;
        let (user, id, command_id) = (Uuid::new_v4(), TestStreamId::new_chat(), Uuid::new_v4());
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        test.stream_subscribe(connection, user, &id).await.unwrap();
        next_stream_message(&mut socket).await;

        let command = TestMessage::new_chat_command("same");
        for _ in 0..2 {
            test.hub
                .send_stream_command(connection, user, &id, command_id, &command)
                .await
                .unwrap();
        }

        let mut received = Vec::new();
        while received.len() < 2 {
            if let StreamMessage::Commands { commands, .. } = &*next_stream_message(&mut socket).await {
                received.extend(commands.iter().map(|c| (c.version, c.command_id)));
            }
        }
        assert_eq!(received, [(1, command_id), (2, command_id)]);
    }

    #[test(serial = "hub-service")]
    async fn the_hub_shutdown_completes_after_the_workers_made_the_commands_durable() {
        let test = create_test_hub().await;
        let user = Uuid::new_v4();
        let id = TestStreamId::new_chat();
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.stream_subscribe(connection, user, &id).await.unwrap();
        next_stream_message(&mut socket).await;
        // Appends wait on this lock (reads do not), taken before the command so that the persistence cannot
        // be done by the time the hub shuts down.
        let log = test.stream_log.as_ref().unwrap();
        let locker = log.pg.direct_pool.get_owned().await.unwrap();
        locker
            .batch_execute("BEGIN; LOCK TABLE cl_commands IN SHARE ROW EXCLUSIVE MODE;")
            .await
            .unwrap();
        test.hub
            .send_stream_command(
                connection,
                user,
                &id,
                Uuid::new_v4(),
                &TestMessage::new_chat_command("last words"),
            )
            .await
            .unwrap();
        let release = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            locker.batch_execute("COMMIT;").await.unwrap();
        });

        test.hub.send_control(ControlCommand::Shutdown).unwrap();
        test.wait_shutdown_complete().await;

        // No waiting for the log: shutdown complete is the promise that the command is there, which can
        // only hold if the hub waited for the lock to be released.
        assert!(
            release.is_finished(),
            "the hub shut down while the persistence was blocked"
        );
        assert_eq!(log.all_commands(&id).await.len(), 1);
        assert_eq!(test.stream_registry.as_ref().unwrap().running_worker_count(), 0);
    }

    #[test(serial = "hub-service")]
    async fn a_disconnect_releases_the_workers_of_the_connection() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let id = TestStreamId::new_chat();
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        test.stream_subscribe(connection, user, &id).await.unwrap();
        next_stream_message(&mut socket).await;
        let streams = test.stream_registry.unwrap().clone();
        assert_eq!(streams.running_worker_count(), 1);

        test.hub
            .request_disconnect(user, connection, DisconnectReason::Client)
            .unwrap();

        timeout(Duration::from_secs(5), async {
            while streams.running_worker_count() != 0 {
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
        let id = TestStreamId::new_pixel_map();
        let off_the_map = TestMessage::new_paint_command(99, 0, 1);
        let send = |stream: &StreamId, command: Vec<u8>| {
            let hub = hub.clone();
            let stream = stream.clone();
            async move {
                hub.send_stream_command(connection, user, &stream, Uuid::new_v4(), &command)
                    .await
            }
        };

        // Before the subscribe.
        assert_eq!(
            send(&id, TestMessage::new_paint_command(1, 1, 1)).await,
            Err(CommandFailure::NotSubscribed)
        );
        test.stream_subscribe(connection, user, &id).await.unwrap();
        // The kind in the stream identity decides what is valid.
        assert_eq!(send(&id, off_the_map).await, Err(CommandFailure::Invalid));
        assert_eq!(
            send(&id, TestMessage::new_chat_command("not a paint")).await,
            Err(CommandFailure::Invalid)
        );
        // After the unsubscribe, with no waiting for the loop in between.
        hub.unsubscribe_stream(connection, id.clone()).unwrap();
        assert_eq!(
            send(&id, TestMessage::new_paint_command(1, 1, 1)).await,
            Err(CommandFailure::NotSubscribed)
        );
    }

    #[test(serial = "hub-service")]
    async fn a_command_into_a_full_inbox_is_refused_as_busy_and_the_connection_stays() {
        let mut test = TestHub::from_env(TestHubOptions {
            with_streams: Some(StreamConfig {
                inbox: InboxConfig {
                    backpressure_low: 0,
                    backpressure_high: 1,
                    backpressure_poll_period: Duration::from_millis(5),
                    backpressure_timeout: Duration::from_millis(100),
                    ..StreamConfig::test_default().inbox
                },
                ..StreamConfig::test_default()
            }),
            ..TestHubOptions::default()
        })
        .await;
        let user = Uuid::new_v4();
        let id = TestStreamId::new_chat();
        let (connection, _socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        // Opened on the registry, not subscribed through the hub: no worker drains the inbox.
        test.stream_registry
            .as_ref()
            .unwrap()
            .open(connection, user, &id)
            .await
            .unwrap();

        let command = TestMessage::new_chat_command("hi");
        let send = || {
            test.hub
                .send_stream_command(connection, user, &id, Uuid::new_v4(), &command)
        };
        send().await.unwrap();
        // The limit is polled, not reserved: the next push is throttled only once the length is seen.
        sleep(Duration::from_millis(100)).await;
        assert_eq!(send().await, Err(CommandFailure::Busy));

        let seen = test.wait_all_commands().await;
        assert!(seen.drops().is_empty(), "{:?}", seen.drops());
        assert_eq!(test.hub.stats().await.connections, 1);
    }

    #[test(serial = "hub-service")]
    async fn a_connection_that_is_gone_is_not_subscribed_to_anything() {
        let mut test = create_test_hub().await;
        let user = Uuid::new_v4();
        let id = TestStreamId::new_chat();
        let (connection, mut socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        test.stream_subscribe(connection, user, &id).await.unwrap();
        next_stream_message(&mut socket).await;

        test.hub
            .request_disconnect(user, connection, DisconnectReason::Client)
            .unwrap();
        test.wait_all_commands().await;

        let result = test
            .hub
            .send_stream_command(
                connection,
                user,
                &id,
                Uuid::new_v4(),
                &TestMessage::new_chat_command("ghost"),
            )
            .await;
        assert_eq!(result, Err(CommandFailure::NotSubscribed));
    }

    #[test(serial = "hub-service")]
    async fn a_stream_outside_the_allow_list_is_refused_as_unauthorized() {
        let mut test = TestHub::from_env(TestHubOptions {
            with_streams: Some(StreamConfig {
                access: StreamAccessConfig::AllowList { ids: vec!["open".to_string()] },
                ..StreamConfig::test_default()
            }),
            ..TestHubOptions::default()
        })
        .await;
        let user = Uuid::new_v4();
        let (connection, _socket) = test.connect_ping(user, 32);
        test.wait_all_commands().await;
        let allowed = TestStreamId::new_chat_with("open");
        let refused = TestStreamId::new_chat_with("closed");

        assert_eq!(test.stream_subscribe(connection, user, &allowed).await, Ok(()));
        assert_eq!(
            test.stream_subscribe(connection, user, &refused).await,
            Err(SubscribeFailure::Unauthorized)
        );

        // nothing was opened for the refused stream, so the connection cannot push to it either
        let result = test
            .hub
            .send_stream_command(
                connection,
                user,
                &refused,
                Uuid::new_v4(),
                &TestMessage::new_chat_command("no"),
            )
            .await;
        assert_eq!(result, Err(CommandFailure::NotSubscribed));
    }
}

#[test(serial = "hub-service")]
async fn a_hub_without_streams_refuses_them() {
    let test = TestHub::default_from_env().await;
    let (connection, _socket) = test.connect_ping(Uuid::new_v4(), 8);
    let id = TestStreamId::new_chat_with("s1");

    assert_eq!(
        test.hub.subscribe_stream(connection, Uuid::new_v4(), id.clone()).await,
        Err(SubscribeFailure::Failed)
    );
    assert_eq!(
        test.hub
            .send_stream_command(connection, Uuid::new_v4(), &id, Uuid::new_v4(), b"{}")
            .await,
        Err(CommandFailure::NotSubscribed)
    );
}
