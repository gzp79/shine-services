use super::*;
use crate::{
    models::messages::{DisconnectReason, StreamMessage, TopicKey},
    services::{socket_channel, SocketReceiver},
    testing::{hub::TestSession, TestMessage},
};
use shine_game::replica::StreamId;
use shine_test::test;
use std::sync::Arc;

struct Setup {
    gateway: HubGateway,
    control: mpsc::UnboundedReceiver<ControlMessage>,
}

impl Setup {
    fn new() -> (Self, Arc<Connections>) {
        let connections = Arc::new(Connections::new());
        let (gateway, control) = HubGateway::new(connections.clone());
        (Self { gateway, control }, connections)
    }

    fn requests(&mut self) -> Vec<ControlCommand> {
        std::iter::from_fn(|| self.control.try_recv().ok())
            .map(|(command, _, _)| command)
            .collect()
    }
}

async fn connect(connections: &Connections, capacity: usize, topics: Vec<TopicKey>) -> (Uuid, Uuid, SocketReceiver) {
    let (socket_sender, receiver) = socket_channel(capacity);
    let (user_id, connection_id) = (Uuid::new_v4(), Uuid::new_v4());
    connections
        .register(
            user_id,
            connection_id,
            TestSession::new_random_key(),
            socket_sender,
            topics,
        )
        .await;
    (user_id, connection_id, receiver)
}

#[test]
async fn a_control_command_reaches_the_loop_with_its_stamp() {
    let (mut setup, _) = Setup::new();

    setup.gateway.send_control(ControlCommand::Shutdown).unwrap();

    let (command, _span, queued_at) = setup.control.try_recv().unwrap();
    assert!(matches!(command, ControlCommand::Shutdown));
    assert!(queued_at.elapsed().as_secs() < 5);
}

#[test]
async fn a_control_command_fails_once_the_loop_is_gone() {
    let (setup, _) = Setup::new();
    let Setup { gateway, control } = setup;
    drop(control);

    assert!(gateway.send_control(ControlCommand::Shutdown).is_err());
}

#[test]
async fn a_message_reaches_its_connection() {
    let (mut setup, connections) = Setup::new();
    let (_, connection_id, mut receiver) = connect(&connections, 4, vec![TopicKey::Ping]).await;

    let queued = setup
        .gateway
        .send_to_connection(connection_id, TestMessage::new_ping("1"))
        .await
        .unwrap();

    assert!(queued);
    assert!(matches!(receiver.try_recv(), Ok(HubMessage::Ping(ping)) if ping.id == "1"));
    assert!(setup.requests().is_empty());
}

#[test]
async fn a_dead_connection_is_requested_once_however_many_sends_find_it() {
    let (mut setup, connections) = Setup::new();
    let (user_id, connection_id, _receiver) = connect(&connections, 1, vec![TopicKey::Ping]).await;
    assert!(setup
        .gateway
        .send_to_connection(connection_id, TestMessage::new_ping("1"))
        .await
        .unwrap());

    for idx in 0..4 {
        let queued = setup
            .gateway
            .send_to_connection(connection_id, TestMessage::new_ping(format!("full-{idx}")))
            .await
            .unwrap();
        assert!(!queued);
    }
    setup
        .gateway
        .send_to_user(user_id, TestMessage::new_ping("user"))
        .await
        .unwrap();
    setup.gateway.broadcast(TestMessage::new_ping("all")).await.unwrap();

    let requests = setup.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert!(matches!(
        requests[0],
        ControlCommand::DisconnectUser { user_id: user, connection_id: connection, reason: DisconnectReason::SlowConsumer }
            if user == user_id && connection == connection_id
    ));
}

#[test]
async fn a_broadcast_requests_every_dead_connection() {
    let (mut setup, connections) = Setup::new();
    let (_, _, receiver_a) = connect(&connections, 4, vec![TopicKey::Ping]).await;
    let (_, _, receiver_b) = connect(&connections, 4, vec![TopicKey::Ping]).await;
    let (_, _, _alive) = connect(&connections, 4, vec![TopicKey::Ping]).await;
    drop(receiver_a);
    drop(receiver_b);

    setup.gateway.broadcast(TestMessage::new_ping("all")).await.unwrap();

    let requests = setup.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert!(requests.iter().all(|request| matches!(
        request,
        ControlCommand::DisconnectUser {
            reason: DisconnectReason::DeadChannel,
            ..
        }
    )));
}

#[test]
async fn a_failed_control_command_is_an_error_of_the_send() {
    let (setup, connections) = Setup::new();
    let (_, connection_id, receiver) = connect(&connections, 4, vec![TopicKey::Ping]).await;
    let Setup { gateway, control } = setup;
    drop(control);
    drop(receiver);

    assert!(gateway
        .send_to_connection(connection_id, TestMessage::new_ping("1"))
        .await
        .is_err());
}

#[test]
async fn the_gateway_delivers_stream_messages_to_the_workers_connections() {
    let (mut setup, connections) = Setup::new();
    let (_, connection_id, mut receiver) = connect(&connections, 4, vec![]).await;
    let stream_id = StreamId::parse("chat:t-1").unwrap();
    let message = Arc::new(StreamMessage::Unsynced { stream_id: stream_id.clone() });

    assert!(StreamEgress::send(&setup.gateway, connection_id, message).await);
    assert!(
        !StreamEgress::send(
            &setup.gateway,
            Uuid::new_v4(),
            Arc::new(StreamMessage::Unsynced { stream_id: stream_id.clone() })
        )
        .await
    );
    assert!(matches!(receiver.try_recv(), Ok(HubMessage::StreamMessage(_))));

    StreamEgress::notify_failed(&setup.gateway, &stream_id).await;
    let requests = setup.requests();
    assert!(
        matches!(requests.as_slice(), [ControlCommand::StreamFailed { .. }]),
        "{requests:?}"
    );
}

#[test]
async fn the_count_follows_the_connections() {
    let (setup, connections) = Setup::new();
    assert_eq!(setup.gateway.connection_count().await, 0);

    let _first = connect(&connections, 1, vec![]).await;
    let _second = connect(&connections, 1, vec![]).await;

    assert_eq!(setup.gateway.connection_count().await, 2);
}
