//! The ws socket of a hub connection: the frames sent to the client, and the hub state its lifecycle
//! leaves behind. The socket is driven through in-memory channels instead of a network connection.

use super::*;
use crate::{
    models::messages::{DisconnectReason, HubEvent},
    services::hub_test_utils::{
        barrier, create_test_hub, dropped, drops, key, lifecycle_of, ping, wait_event, wait_stopped, Lifecycle,
        TestHub, TestHubOptions, BARRIER_TIMEOUT,
    },
};
use futures::channel::mpsc as socket;
use shine_test::test;
use std::convert::Infallible;
use tokio::{task::JoinHandle, time::timeout};

fn close(code: u16, reason: &'static str) -> WsMessage {
    WsMessage::Close(Some(CloseFrame { code, reason: reason.into() }))
}

/// The event that ends the socket of connection `Uuid::nil()`.
fn dropped_event(reason: DisconnectReason) -> HubMessage {
    HubMessage::Hub(HubEvent::UserDisconnected {
        user_id: Uuid::nil(),
        connection_id: Uuid::nil(),
        reason,
    })
}

/// The ids and texts of a chat frame.
fn ping_frame(frame: &WsMessage) -> Vec<String> {
    let WsMessage::Text(text) = frame else {
        panic!("expected a text frame, got {frame:?}");
    };
    let json: serde_json::Value = serde_json::from_str(text.as_str()).unwrap();
    assert_eq!(json["type"], "ping");
    json["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().into())
        .collect()
}

/// Runs [`forward_egress`] over the given hub messages and returns the frames it sent.
async fn forwarded(messages: Vec<HubMessage>) -> Vec<WsMessage> {
    let (tx, rx) = mpsc::channel(messages.len().max(1));
    for message in messages {
        tx.try_send(message).unwrap();
    }
    drop(tx);
    let (frames_tx, frames_rx) = socket::unbounded();
    forward_egress(rx, frames_tx, Uuid::nil(), Uuid::nil()).await;
    frames_rx.collect().await
}

#[test]
async fn a_close_notice_ends_the_socket_with_its_reason() {
    let frames = forwarded(vec![
        ping("1-0"),
        HubMessage::Hub(HubEvent::Shutdown),
        dropped_event(DisconnectReason::Replaced),
        ping("2-0"),
    ])
    .await;

    // Hub events are not forwarded, and nothing after the notice is.
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(ping_frame(&frames[0]), ["1-0".to_string()]);
    assert_eq!(frames[1], close(CLOSE_CODE_DROPPED, "replaced"));
}

#[test]
async fn the_drops_of_other_connections_do_not_end_the_socket() {
    let other = HubMessage::Hub(HubEvent::UserDisconnected {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
        reason: DisconnectReason::SessionInvalid,
    });
    let frames = forwarded(vec![other, ping("1-0")]).await;

    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(ping_frame(&frames[0]), ["1-0".to_string()]);
    assert_eq!(frames[1], WsMessage::Close(None));
}

#[test]
async fn every_drop_reason_maps_to_its_close_code() {
    for (reason, code) in [
        (DisconnectReason::Replaced, CLOSE_CODE_DROPPED),
        (DisconnectReason::SessionInvalid, CLOSE_CODE_DROPPED),
        (DisconnectReason::HeartbeatInactive, CLOSE_CODE_DROPPED),
        (DisconnectReason::RegistryInactive, CLOSE_CODE_DROPPED),
        (DisconnectReason::SlowConsumer, CLOSE_CODE_DROPPED),
        (DisconnectReason::DeadChannel, CLOSE_CODE_DROPPED),
        (DisconnectReason::Shutdown, close_code::AWAY),
    ] {
        assert_eq!(
            forwarded(vec![dropped_event(reason)]).await,
            [close(code, reason.as_str())],
            "{reason}"
        );
    }
}

#[test]
async fn a_channel_ending_without_a_notice_closes_without_a_reason() {
    // As for a slow consumer, whose notice did not fit: the buffered messages are still sent first.
    let frames = forwarded(vec![ping("1-0")]).await;
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(ping_frame(&frames[0]), ["1-0".to_string()]);
    assert_eq!(frames[1], WsMessage::Close(None));
}

// --- Sockets on a hub ---------------------------------------------------------------------------

struct Client {
    connection_id: Uuid,
    to_server: Option<socket::UnboundedSender<Result<WsMessage, Infallible>>>,
    from_server: socket::UnboundedReceiver<WsMessage>,
    server: JoinHandle<()>,
}

impl Client {
    /// Opens a socket and waits until the hub has registered it.
    async fn open(test: &mut TestHub, user_id: Uuid) -> Self {
        let (to_server, server_rx) = socket::unbounded();
        let (server_tx, from_server) = socket::unbounded();
        let server = tokio::spawn(serve_socket(
            server_tx,
            server_rx,
            user_id,
            key(),
            test.hub.clone(),
            SpanContext::empty_context(),
        ));
        let seen = wait_event(
            &mut test.events,
            BARRIER_TIMEOUT,
            |event| matches!(event, HubEvent::UserConnected { user_id: connected, .. } if *connected == user_id),
        )
        .await
        .expect("the socket was not registered");
        Self {
            connection_id: connection_of(&seen, user_id),
            to_server: Some(to_server),
            from_server,
            server,
        }
    }

    fn send(&self, text: &str) {
        self.to_server
            .as_ref()
            .unwrap()
            .unbounded_send(Ok(WsMessage::Text(text.into())))
            .unwrap();
    }

    /// The client goes away: its side of the socket ends.
    fn leave(&mut self) {
        self.to_server = None;
    }

    /// Waits until the server side has finished, and returns every frame it sent.
    async fn finished(self) -> Vec<WsMessage> {
        timeout(BARRIER_TIMEOUT, self.server)
            .await
            .expect("the server side did not finish")
            .unwrap();
        self.from_server.collect().await
    }

    async fn next_frame(&mut self) -> WsMessage {
        timeout(BARRIER_TIMEOUT, self.from_server.next())
            .await
            .expect("no frame from the server")
            .expect("the socket was closed")
    }
}

/// The connection id of the user's socket, from its connect event.
fn connection_of(events: &[HubEvent], user_id: Uuid) -> Uuid {
    match lifecycle_of(events, user_id).as_slice() {
        [.., Lifecycle::Connected(connection_id)] => *connection_id,
        other => panic!("no connection of {user_id}: {other:?}"),
    }
}

#[test(serial = "hub-service")]
async fn a_replaced_socket_is_closed_with_the_reason() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let user = Uuid::new_v4();

    let first = Client::open(&mut test, user).await;
    let mut second = Client::open(&mut test, user).await;

    assert_eq!(first.finished().await, [close(CLOSE_CODE_DROPPED, "replaced")]);
    test.hub.broadcast(ping("1-0")).await.unwrap();
    assert_eq!(ping_frame(&second.next_frame().await), ["1-0".to_string()]);

    second.leave();
    let second_id = second.connection_id;
    assert_eq!(second.finished().await, [WsMessage::Close(None)]);
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(second_id)]);
}

#[test(serial = "hub-service")]
async fn a_forced_drop_and_a_shutdown_close_the_sockets_with_their_reason() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let (dropped_user, other_user) = (Uuid::new_v4(), Uuid::new_v4());

    let dropped_client = Client::open(&mut test, dropped_user).await;
    let other = Client::open(&mut test, other_user).await;

    test.hub
        .request_drop(
            dropped_user,
            dropped_client.connection_id,
            DisconnectReason::SessionInvalid,
        )
        .unwrap();
    assert_eq!(
        dropped_client.finished().await,
        [close(CLOSE_CODE_DROPPED, "session_invalid")]
    );

    test.hub.request_shutdown().unwrap();
    assert_eq!(other.finished().await, [close(close_code::AWAY, "shutdown")]);
    wait_stopped(&test.hub).await;

    // Once the hub is gone a new socket cannot register, and is closed right away.
    let (to_server, server_rx) = socket::unbounded::<Result<WsMessage, Infallible>>();
    let (server_tx, from_server) = socket::unbounded();
    serve_socket(
        server_tx,
        server_rx,
        Uuid::new_v4(),
        key(),
        test.hub.clone(),
        SpanContext::empty_context(),
    )
    .await;
    drop(to_server);
    assert_eq!(from_server.collect::<Vec<_>>().await, []);
}

#[test(serial = "hub-service")]
async fn a_client_leaving_under_traffic_is_not_counted_as_a_drop() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };

    // Directed sends race the client leaving: the socket must keep its channel until the hub has
    // removed it, or the sends would find it closed and count a dead channel.
    for _ in 0..20 {
        let user = Uuid::new_v4();
        let mut client = Client::open(&mut test, user).await;
        let connection_id = client.connection_id;

        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let sender = {
            let hub = test.hub.clone();
            tokio::spawn(async move {
                let mut seq = 0;
                while !*stop_rx.borrow() {
                    hub.send_to_connection(connection_id, ping(format!("{seq}-0")))
                        .await
                        .unwrap();
                    seq += 1;
                    // Paced, so the egress never fills and gets the socket dropped as slow.
                    tokio::time::sleep(Duration::from_micros(200)).await;
                }
            })
        };
        tokio::time::sleep(Duration::from_millis(5)).await;
        client.leave();
        let frames = client.finished().await;
        let _ = stop_tx.send(true);
        sender.await.unwrap();

        assert_eq!(frames.last(), Some(&WsMessage::Close(None)), "{:?}", frames.last());
        let seen = barrier(&test.hub, &mut test.events).await;
        assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(connection_id)]);
        assert_eq!(drops(&seen), dropped(&[]));
    }
    assert_eq!(test.hub.stats().await.connections, 0);
}

#[test(serial = "hub-service")]
async fn client_ping_is_appended_and_delivered_back() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let user = Uuid::new_v4();
    let mut client = Client::open(&mut test, user).await;

    // Invalid frames are ignored; the valid ping goes through the room and back to every socket.
    client.send("not json");
    client.send(r#"{"type":"unknown"}"#);
    client.send(r#"{"type":"ping","id":"hi"}"#);
    let delivered = ping_frame(&client.next_frame().await);
    assert_eq!(delivered, ["hi".to_string()]);

    client.leave();
    assert_eq!(client.finished().await, [WsMessage::Close(None)]);
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(drops(&seen), dropped(&[]));
}
