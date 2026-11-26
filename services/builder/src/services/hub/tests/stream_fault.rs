//! What a stream client sees while the Postgres or the Redis behind the streams is gone, and that it can carry
//! on once they are back. The sockets of the clients are kept in both cases: they are not dropped for it.

use crate::{
    models::messages::{CommandFailure, HubMessage, StreamMessage, SubscribeFailure},
    services::command_stream::{InboxConfig, LogConfig, StreamConfig},
    testing::{
        command_stream::TestStreamId,
        hub::{TestEgress, TestHub, TestHubOptions},
        TestMessage,
    },
};
use shine_test::{test, toxiproxy::Toxiproxy};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

/// Quick retries of a few milliseconds around pools that give up after a second. The inbox is looked at again
/// every 100 ms, as the wake of a Redis that was gone is not delivered.
fn config(retries: u32) -> StreamConfig {
    let base = StreamConfig::test_default();
    StreamConfig {
        inbox: InboxConfig {
            max_retries: retries,
            recovery_poll_interval: Duration::from_millis(100),
            retry_min_delay: Duration::from_millis(20),
            retry_max_delay: Duration::from_millis(100),
            ..base.inbox
        },
        log: LogConfig {
            max_retries: retries,
            retry_min_delay: Duration::from_millis(20),
            retry_max_delay: Duration::from_millis(100),
            ..base.log
        },
        ..base
    }
}

async fn hub(retries: u32) -> TestHub {
    TestHub::from_env(TestHubOptions {
        fast_failing_stores: true,
        with_streams: Some(config(retries)),
        ..TestHubOptions::default()
    })
    .await
}

/// The next stream message the connection gets within `wait`.
async fn next_stream_message(socket: &mut TestEgress, wait: Duration) -> Option<Arc<StreamMessage>> {
    timeout(wait, async {
        loop {
            match socket.recv().await {
                Some(HubMessage::StreamMessage(message)) => return message,
                Some(_) => {}
                None => panic!("the connection was closed"),
            }
        }
    })
    .await
    .ok()
}

#[test(serial = "hub-service")]
async fn subscribing_while_postgres_is_gone_is_told_it_failed_and_works_after_it_is_back() {
    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;
    let mut test = hub(2).await;
    let (user, id) = (Uuid::new_v4(), TestStreamId::new_chat());
    let (connection, mut socket) = test.connect_ping(user, 32);
    test.wait_all_commands().await;

    let postgres = toxiproxy.proxy("postgres").await;
    postgres.set_enabled(false).await;
    let result = test.stream_subscribe(connection, user, &id).await;

    // The subscribe is refused at once, or the worker gives up on loading the stream: told either way.
    match result {
        Err(reason) => assert_eq!(reason, SubscribeFailure::Failed),
        Ok(()) => {
            let message = next_stream_message(&mut socket, Duration::from_secs(30))
                .await
                .expect("no answer");
            assert!(matches!(&*message, StreamMessage::Desynced { .. }), "{message:?}");
        }
    }
    // Nothing is subscribed, and the connection is still there.
    let command = TestMessage::new_chat_command("after the failure");
    let sent = test
        .hub
        .send_stream_command(connection, user, &id, Uuid::new_v4(), &command)
        .await;
    assert_eq!(sent, Err(CommandFailure::NotSubscribed));
    assert_eq!(test.hub.stats().await.connections, 1);

    postgres.set_enabled(true).await;
    test.stream_subscribe(connection, user, &id).await.unwrap();
    let message = next_stream_message(&mut socket, Duration::from_secs(10))
        .await
        .expect("no sync");
    assert!(
        matches!(&*message, StreamMessage::Sync { commands, .. } if commands.is_empty()),
        "{message:?}"
    );
}

#[test(serial = "hub-service")]
async fn subscribing_while_postgres_is_gone_for_a_moment_resumes_after_it_is_back() {
    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;
    let mut test = hub(20).await;
    let (user, id) = (Uuid::new_v4(), TestStreamId::new_chat());
    let (connection, mut socket) = test.connect_ping(user, 32);
    test.wait_all_commands().await;

    let postgres = toxiproxy.proxy("postgres").await;
    postgres.set_enabled(false).await;
    let subscribed = test.stream_subscribe(connection, user, &id).await;
    // Nothing is served while it is gone, and the stream is not given up either.
    assert!(next_stream_message(&mut socket, Duration::from_millis(500))
        .await
        .is_none());
    postgres.set_enabled(true).await;

    assert_eq!(subscribed, Ok(()));
    let message = next_stream_message(&mut socket, Duration::from_secs(30))
        .await
        .expect("no answer");
    assert!(
        matches!(&*message, StreamMessage::Sync { commands, .. } if commands.is_empty()),
        "{message:?}"
    );
}

#[test(serial = "hub-service")]
async fn a_command_while_redis_is_gone_is_refused_as_failed_and_the_stream_carries_on_after() {
    let toxiproxy = Toxiproxy::from_env();
    let _reset = toxiproxy.reset_guard().await;
    let mut test = hub(20).await;
    let (user, id) = (Uuid::new_v4(), TestStreamId::new_chat());
    let (connection, mut socket) = test.connect_ping(user, 32);
    test.wait_all_commands().await;
    test.stream_subscribe(connection, user, &id).await.unwrap();
    next_stream_message(&mut socket, Duration::from_secs(10))
        .await
        .expect("no sync");

    let redis = toxiproxy.proxy("redis").await;
    redis.set_enabled(false).await;
    let command = TestMessage::new_chat_command("down");
    let sent = test
        .hub
        .send_stream_command(connection, user, &id, Uuid::new_v4(), &command)
        .await;
    assert_eq!(sent, Err(CommandFailure::Failed));
    redis.set_enabled(true).await;

    // A refused command may or may not have been queued, so a later one is looked for by its content. The
    // worker looks at the inbox again at the latest on its recovery poll.
    let up = TestMessage::new_chat_command("up");
    let mut delivered = false;
    for _ in 0..30 {
        let _ = test
            .hub
            .send_stream_command(connection, user, &id, Uuid::new_v4(), &up)
            .await;
        if let Some(message) = next_stream_message(&mut socket, Duration::from_secs(1)).await {
            if matches!(&*message, StreamMessage::Commands { commands, .. } if commands.iter().any(|c| c.command == up))
            {
                delivered = true;
                break;
            }
        }
    }
    assert!(delivered, "the stream did not carry on after Redis");
    assert_eq!(test.hub.stats().await.connections, 1);
}
