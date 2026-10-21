//! Stream subscriptions through the hub's command loop.

use crate::{
    models::messages::{HubMessage, StreamEvent, StreamResume, TopicKey},
    services::hub_test_utils::{barrier, connect, create_test_hub, egress_state, key, open, TestHubOptions},
};
use shine_infra::db::command_log::StreamId;
use shine_test::test;
use tokio::sync::mpsc::{self, error::TryRecvError, UnboundedReceiver};
use uuid::Uuid;

fn chat_stream() -> StreamId {
    StreamId::parse("chat-global").unwrap()
}

fn stream_events(rx: &mut UnboundedReceiver<HubMessage>) -> Vec<StreamEvent> {
    let mut events = vec![];
    loop {
        match rx.try_recv() {
            Ok(HubMessage::StreamEvent(event)) => events.push(event),
            Ok(_) => {}
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return events,
        }
    }
}

#[test(serial = "hub-service")]
async fn subscriptions_are_published_for_registered_connections_only() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let mut streams = test.hub.subscribe(vec![TopicKey::Stream]).await;
    let user_id = Uuid::new_v4();
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);
    let stream_id = chat_stream();

    // An unknown connection, then the live one, then the same connection after it disconnected:
    // only the live one counts, so a late subscribe cannot pin a stream worker.
    test.hub
        .request_subscribe_stream(user_id, Uuid::new_v4(), stream_id.clone(), "chat", None)
        .unwrap();
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "chat", None)
        .unwrap();
    test.hub
        .request_unsubscribe_stream(user_id, connection_id, stream_id.clone())
        .unwrap();
    test.hub.request_disconnection(user_id, connection_id).unwrap();
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "chat", None)
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        stream_events(&mut streams),
        vec![
            StreamEvent::Subscribed {
                connection_id,
                stream_id: stream_id.clone(),
                kind: "chat",
                resume: None
            },
            StreamEvent::Unsubscribed { connection_id, stream_id },
        ]
    );
}

#[test(serial = "hub-service")]
async fn a_subscribe_carries_the_resume_position() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let mut streams = test.hub.subscribe(vec![TopicKey::Stream]).await;
    let user_id = Uuid::new_v4();
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);
    let resume = StreamResume { generation: 3, version: 7 };

    test.hub
        .request_subscribe_stream(user_id, connection_id, chat_stream(), "chat", Some(resume))
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(
        stream_events(&mut streams),
        vec![StreamEvent::Subscribed {
            connection_id,
            stream_id: chat_stream(),
            kind: "chat",
            resume: Some(resume)
        }]
    );
}

#[test(serial = "hub-service")]
async fn unsubscribing_keeps_the_socket_connected() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let user_id = Uuid::new_v4();
    let (connection_id, mut egress) = connect(&test.hub, user_id, 16);
    test.hub
        .request_subscribe_stream(user_id, connection_id, chat_stream(), "chat", None)
        .unwrap();
    test.hub
        .request_unsubscribe_stream(user_id, connection_id, chat_stream())
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert_eq!(test.hub.stats().await.connections, 1);
    assert_eq!(egress_state(&mut egress), open(), "the connection stays open");

    test.hub.request_disconnection(user_id, connection_id).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn stream_events_reach_only_internal_subscribers() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let mut hub_events = test.hub.subscribe(vec![TopicKey::Hub]).await;
    let user_id = Uuid::new_v4();
    let (connection_id, mut egress) = connect(&test.hub, user_id, 16);

    test.hub
        .request_subscribe_stream(user_id, connection_id, chat_stream(), "chat", None)
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    assert!(
        stream_events(&mut hub_events).is_empty(),
        "a Hub subscriber does not see stream events"
    );
    assert!(
        std::iter::from_fn(|| egress.try_recv().ok()).all(|m| !matches!(m, HubMessage::StreamEvent(_))),
        "a socket does not see stream events"
    );

    test.hub.request_disconnection(user_id, connection_id).unwrap();
    barrier(&test.hub, &mut test.events).await;
}

#[test(serial = "hub-service")]
async fn a_socket_replies_through_a_weak_sender_without_keeping_its_channel_open() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let user_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel(16);
    let weak = tx.downgrade();
    let connection_id = test
        .hub
        .request_connection(user_id, key(), tx, vec![TopicKey::Stream])
        .unwrap();

    // The socket replies at once, before the command loop registered the connection.
    let reply = HubMessage::Stream(crate::models::messages::StreamMessage::Reset { stream_id: chat_stream() });
    weak.upgrade()
        .expect("the hub still holds the channel")
        .try_send(reply)
        .unwrap();

    test.hub.request_disconnection(user_id, connection_id).unwrap();
    barrier(&test.hub, &mut test.events).await;

    // The hub dropped its sender: the reply is still there, then the channel ends, and the weak
    // sender cannot revive it.
    assert!(matches!(rx.try_recv(), Ok(HubMessage::Stream(_))));
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Disconnected)));
    assert!(weak.upgrade().is_none());
}
