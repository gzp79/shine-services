//! The ws socket of a hub connection: the frames sent to the client, and the hub state its lifecycle
//! leaves behind. The socket is driven through in-memory channels instead of a network connection.

use super::*;
use crate::{
    models::messages::{ChatBatch, ChatComment, DisconnectReason, HubEvent},
    repositories::chat_comments::redis::RedisChatCommentsDb,
    services::hub_test_utils::{
        barrier, create_test_hub, dropped, drops, key, lifecycle_of, wait_event, wait_stopped, Lifecycle, TestHub,
        TestHubOptions, BARRIER_TIMEOUT,
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

fn chat_payload(id: &str, text: &str) -> HubMessage {
    HubMessage::Chat(ChatBatch {
        comments: vec![ChatComment {
            id: id.into(),
            user_id: Uuid::nil(),
            text: text.into(),
        }].into(),
    })
}

/// The ids and texts of a chat frame.
fn chat_frame(frame: &WsMessage) -> Vec<(String, String)> {
    let WsMessage::Text(text) = frame else {
        panic!("expected a text frame, got {frame:?}");
    };
    let json: serde_json::Value = serde_json::from_str(text.as_str()).unwrap();
    assert_eq!(json["type"], "chat");
    json["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["id"].as_str().unwrap().into(), m["text"].as_str().unwrap().into()))
        .collect()
}

/// Runs [`forward_egress`] over the given hub messages and returns the frames it sent. The client is
/// still there when the hub ends the connection.
async fn forwarded(messages: Vec<HubMessage>) -> Vec<WsMessage> {
    forwarded_with(messages, false).await
}

async fn forwarded_with(messages: Vec<HubMessage>, client_left: bool) -> Vec<WsMessage> {
    let (tx, rx) = mpsc::channel(messages.len().max(1));
    for message in messages {
        tx.try_send(message).unwrap();
    }
    drop(tx);
    let (_notice_tx, notice_rx) = mpsc::channel(1);
    let (frames_tx, frames_rx) = socket::unbounded();
    forward_egress(
        rx,
        notice_rx,
        frames_tx,
        Uuid::nil(),
        Uuid::nil(),
        Arc::new(AtomicBool::new(client_left)),
        SocketTimings::default(),
    )
    .await;
    frames_rx.collect().await
}

#[test]
async fn a_close_notice_ends_the_socket_with_its_reason() {
    let frames = forwarded(vec![
        chat_payload("1-0", "hello"),
        HubMessage::Hub(HubEvent::Shutdown),
        dropped_event(DisconnectReason::Replaced),
        chat_payload("2-0", "after the close"),
    ])
    .await;

    // Hub events are not forwarded, and nothing after the notice is.
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(chat_frame(&frames[0]), [("1-0".to_string(), "hello".to_string())]);
    assert_eq!(frames[1], close(CLOSE_CODE_REPLACED, "replaced"));
}

#[test]
async fn the_drops_of_other_connections_do_not_end_the_socket() {
    let other = HubMessage::Hub(HubEvent::UserDisconnected {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
        reason: DisconnectReason::SessionInvalid,
    });
    let frames = forwarded_with(vec![other, chat_payload("1-0", "still open")], true).await;

    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(chat_frame(&frames[0]), [("1-0".to_string(), "still open".to_string())]);
    assert_eq!(frames[1], WsMessage::Close(None));
}

#[test]
async fn every_drop_reason_maps_to_its_close_code() {
    for (reason, code) in [
        (DisconnectReason::Replaced, 4001),
        (DisconnectReason::SessionInvalid, 4002),
        (DisconnectReason::HeartbeatInactive, 4003),
        (DisconnectReason::RegistryInactive, 4003),
        (DisconnectReason::DeadChannel, 4003),
        (DisconnectReason::SlowConsumer, close_code::AGAIN),
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
    // As for a slow consumer, whose notice did not fit: the buffered messages are still sent first,
    // and the client is told to retry later.
    let frames = forwarded(vec![chat_payload("1-0", "buffered")]).await;
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(chat_frame(&frames[0]), [("1-0".to_string(), "buffered".to_string())]);
    assert_eq!(frames[1], close(close_code::AGAIN, "try_again_later"));

    // A client that left needs no reason.
    let frames = forwarded_with(vec![chat_payload("1-0", "buffered")], true).await;
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
    async fn open(test: &mut TestHub, chat: &ChatService, user_id: Uuid) -> Self {
        Self::open_with(test, chat, user_id, SocketTimings::default()).await
    }

    async fn open_with(test: &mut TestHub, chat: &ChatService, user_id: Uuid, timings: SocketTimings) -> Self {
        let (to_server, server_rx) = socket::unbounded();
        let (server_tx, from_server) = socket::unbounded();
        let server = tokio::spawn(serve_socket(
            server_tx,
            server_rx,
            user_id,
            key(),
            test.hub.clone(),
            chat.clone(),
            SpanContext::empty_context(),
            timings,
        ));
        let seen = wait_event(
            &mut test.events,
            BARRIER_TIMEOUT,
            |event| matches!(event, HubEvent::UserConnected { user_id: connected, .. } if *connected == user_id),
        )
        .await
        .expect("the socket was not registered");
        let mut client = Self {
            connection_id: connection_of(&seen, user_id),
            to_server: Some(to_server),
            from_server,
            server,
        };
        // Every connection starts with the hello frame.
        let hello = client.next_json().await;
        assert_eq!(hello["type"], "hello");
        assert_eq!(hello["version"], PROTOCOL_VERSION);
        assert_eq!(hello["connectionId"], client.connection_id.to_string());
        client
    }

    async fn next_json(&mut self) -> serde_json::Value {
        match self.next_frame().await {
            WsMessage::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
            frame => panic!("expected a text frame, got {frame:?}"),
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

async fn socket_hub() -> Option<(TestHub, ChatService)> {
    let test = create_test_hub(TestHubOptions::default()).await?;
    let chat_db = RedisChatCommentsDb::new(&test.redis, 200, 3600).await.unwrap();
    let chat = ChatService::with_room(chat_db, test.chat_room.clone());
    Some((test, chat))
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
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let user = Uuid::new_v4();

    let first = Client::open(&mut test, &chat, user).await;
    let mut second = Client::open(&mut test, &chat, user).await;

    assert_eq!(first.finished().await, [close(CLOSE_CODE_REPLACED, "replaced")]);
    test.hub.broadcast(chat_payload("1-0", "still here")).await.unwrap();
    assert_eq!(
        chat_frame(&second.next_frame().await),
        [("1-0".to_string(), "still here".to_string())]
    );

    second.leave();
    let second_id = second.connection_id;
    assert_eq!(second.finished().await, [WsMessage::Close(None)]);
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(second_id)]);
}

#[test(serial = "hub-service")]
async fn a_forced_drop_and_a_shutdown_close_the_sockets_with_their_reason() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let (dropped_user, other_user) = (Uuid::new_v4(), Uuid::new_v4());

    let dropped_client = Client::open(&mut test, &chat, dropped_user).await;
    let other = Client::open(&mut test, &chat, other_user).await;

    test.hub
        .request_drop(
            dropped_user,
            dropped_client.connection_id,
            DisconnectReason::SessionInvalid,
        )
        .unwrap();
    assert_eq!(
        dropped_client.finished().await,
        [close(CLOSE_CODE_SESSION_INVALID, "session_invalid")]
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
        chat.clone(),
        SpanContext::empty_context(),
        SocketTimings::default(),
    )
    .await;
    drop(to_server);
    assert_eq!(from_server.collect::<Vec<_>>().await, []);
}

#[test(serial = "hub-service")]
async fn a_client_leaving_under_traffic_is_not_counted_as_a_drop() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };

    // Directed sends race the client leaving: the socket must keep its channel until the hub has
    // removed it, or the sends would find it closed and count a dead channel.
    for _ in 0..20 {
        let user = Uuid::new_v4();
        let mut client = Client::open(&mut test, &chat, user).await;
        let connection_id = client.connection_id;

        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let sender = {
            let hub = test.hub.clone();
            tokio::spawn(async move {
                let mut seq = 0;
                while !*stop_rx.borrow() {
                    hub.send_to_connection(connection_id, chat_payload(&format!("{seq}-0"), ""))
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
async fn client_chat_is_appended_and_delivered_back() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let user = Uuid::new_v4();
    let mut client = Client::open(&mut test, &chat, user).await;

    // Refused frames are answered with an error; the valid chat is acknowledged and goes through
    // the room and back to every socket.
    client.send("not json");
    assert_eq!(client.next_json().await, serde_json::json!({"type": "error", "code": "invalid"}));
    client.send(r#"{"type":"unknown"}"#);
    assert_eq!(client.next_json().await, serde_json::json!({"type": "error", "code": "invalid"}));
    client.send(r#"{"type":"chat","text":"   "}"#);
    assert_eq!(client.next_json().await, serde_json::json!({"type": "error", "code": "invalid"}));
    client.send(r#"{"type":"chat","text":"hi"}"#);
    let mut ack = None;
    let mut delivered = None;
    while ack.is_none() || delivered.is_none() {
        let frame = client.next_json().await;
        match frame["type"].as_str().unwrap() {
            "ack" => ack = Some(frame["id"].as_str().unwrap().to_string()),
            "chat" => delivered = Some(frame["messages"].clone()),
            other => panic!("unexpected frame {other}: {frame}"),
        }
    }
    let delivered = delivered.unwrap();
    assert_eq!(delivered.as_array().unwrap().len(), 1);
    assert_eq!(delivered[0]["text"], "hi");
    assert_eq!(delivered[0]["id"].as_str(), ack.as_deref(), "the ack names the stored comment");

    client.leave();
    assert_eq!(client.finished().await, [WsMessage::Close(None)]);
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(drops(&seen), dropped(&[]));
}

#[test(serial = "hub-service")]
async fn a_socket_task_that_is_cancelled_leaves_no_connection_behind() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let user = Uuid::new_v4();
    let client = Client::open(&mut test, &chat, user).await;
    let connection_id = client.connection_id;
    assert_eq!(test.hub.stats().await.connections, 1);

    // The task is dropped mid-flight, as on a runtime shutdown or a panic in the socket code.
    client.server.abort();
    let seen = wait_event(
        &mut test.events,
        BARRIER_TIMEOUT,
        |event| matches!(event, HubEvent::UserDisconnected { connection_id: closed, .. } if *closed == connection_id),
    )
    .await
    .expect("the hub still holds the connection of a cancelled socket");

    assert_eq!(lifecycle_of(&seen, user), [Lifecycle::Disconnected(connection_id)]);
    assert_eq!(drops(&seen), dropped(&[]));
    assert_eq!(test.hub.stats().await.connections, 0);
}

#[test(serial = "hub-service")]
async fn oversized_chat_is_refused_and_not_stored() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let mut client = Client::open(&mut test, &chat, Uuid::new_v4()).await;

    let text = "x".repeat(MAX_CHAT_TEXT_BYTES + 1);
    client.send(&serde_json::json!({"type": "chat", "text": text}).to_string());
    assert_eq!(
        client.next_json().await,
        serde_json::json!({"type": "error", "code": "too_long"})
    );
    assert!(chat.list_after("0", 10).await.unwrap().comments.is_empty());

    let text = "x".repeat(MAX_CHAT_TEXT_BYTES);
    client.send(&serde_json::json!({"type": "chat", "text": text}).to_string());
    assert_eq!(client.next_json().await["type"], "ack");
    client.leave();
    client.finished().await;
}

#[test(serial = "hub-service")]
async fn chat_faster_than_the_rate_limit_is_refused() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let mut client = Client::open(&mut test, &chat, Uuid::new_v4()).await;

    // The burst is accepted, the rest refused until tokens are refilled.
    for i in 0..CHAT_BURST + 2 {
        client.send(&serde_json::json!({"type": "chat", "text": format!("m{i}")}).to_string());
    }
    let (mut acks, mut limited) = (0, 0);
    while acks + limited < CHAT_BURST + 2 {
        let frame = client.next_json().await;
        match (frame["type"].as_str().unwrap(), frame["code"].as_str()) {
            ("ack", _) => acks += 1,
            ("error", Some("rate_limited")) => limited += 1,
            ("chat", _) => {}
            other => panic!("unexpected frame {other:?}"),
        }
    }
    assert_eq!((acks, limited), (CHAT_BURST, 2));
    assert_eq!(chat.list_after("0", 100).await.unwrap().comments.len(), CHAT_BURST as usize);
    client.leave();
    client.finished().await;
}

#[test(serial = "hub-service")]
async fn a_client_that_keeps_sending_refused_frames_is_closed() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let user = Uuid::new_v4();
    let client = Client::open(&mut test, &chat, user).await;
    let connection_id = client.connection_id;

    for _ in 0..MAX_STRIKES {
        client.send("not json");
    }
    let frames = client.finished().await;
    assert_eq!(frames.last(), Some(&close(close_code::POLICY, "abusive_client")), "{frames:?}");

    let seen = wait_event(
        &mut test.events,
        BARRIER_TIMEOUT,
        |event| matches!(event, HubEvent::UserDisconnected { connection_id: closed, .. } if *closed == connection_id),
    )
    .await;
    assert!(seen.is_some() || test.hub.stats().await.connections == 0);
    assert_eq!(test.hub.stats().await.connections, 0);
}

#[test(serial = "hub-service")]
async fn a_silent_client_is_closed_after_the_idle_timeout_and_pinged_meanwhile() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let timings = SocketTimings {
        ping_interval: Duration::from_millis(50),
        idle_timeout: Duration::from_millis(400),
        ..SocketTimings::default()
    };
    let mut client = Client::open_with(&mut test, &chat, Uuid::new_v4(), timings).await;

    assert!(matches!(client.next_frame().await, WsMessage::Ping(_)), "a quiet client is pinged");
    let frames = client.finished().await;
    assert!(
        frames.iter().filter(|frame| matches!(frame, WsMessage::Ping(_))).count() >= 1,
        "{frames:?}"
    );
    assert_eq!(test.hub.stats().await.connections, 0, "the idle socket is removed from the hub");
}

/// A sink that never accepts a frame, as a peer with a closed TCP window.
struct StuckSink;

impl Sink<WsMessage> for StuckSink {
    type Error = Infallible;

    fn poll_ready(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Pending
    }

    fn start_send(self: std::pin::Pin<&mut Self>, _: WsMessage) -> Result<(), Infallible> {
        Ok(())
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Pending
    }

    fn poll_close(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Pending
    }
}

#[test(serial = "hub-service")]
async fn a_client_that_does_not_read_cannot_pin_the_socket() {
    let Some((test, chat)) = socket_hub().await else {
        return;
    };
    let user = Uuid::new_v4();
    let (_to_server, server_rx) = socket::unbounded::<Result<WsMessage, Infallible>>();
    let timings = SocketTimings {
        send_timeout: Duration::from_millis(100),
        ..SocketTimings::default()
    };
    let server = tokio::spawn(serve_socket(
        StuckSink,
        server_rx,
        user,
        key(),
        test.hub.clone(),
        chat.clone(),
        SpanContext::empty_context(),
        timings,
    ));

    timeout(BARRIER_TIMEOUT, server)
        .await
        .expect("a stuck write must not keep the socket alive")
        .unwrap();
    timeout(BARRIER_TIMEOUT, async {
        while test.hub.stats().await.connections != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the connection of the stuck socket must be removed from the hub");
}
