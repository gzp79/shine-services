//! The ws socket of a hub connection: the frames sent to the client, and the hub state its lifecycle
//! leaves behind. The socket is driven through in-memory channels instead of a network connection.

use super::*;
use crate::{
    models::messages::{DisconnectReason, HubEvent},
    routes::ws::ws_config::SocketLimits,
    services::StreamConfig,
    testing::{
        command_stream::TestStreamId,
        hub::{HubEvents, Lifecycle, TestHub, TestHubOptions, TestSession},
        TestMessage,
    },
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::channel::mpsc as socket;
use serde::Deserialize;
use shine_infra::sync::RateLimit;
use shine_test::test;
use std::{collections::BTreeMap, convert::Infallible};
use tokio::{
    task::JoinHandle,
    time::{timeout, Duration},
};

const WAIT_TIMEOUT: Duration = Duration::from_secs(30);

fn close(code: u16, reason: &'static str) -> WsMessage {
    WsMessage::Close(Some(CloseFrame { code, reason: reason.into() }))
}

fn text_json(frame: &WsMessage) -> serde_json::Value {
    let WsMessage::Text(text) = frame else {
        panic!("expected a text frame, got {frame:?}");
    };
    serde_json::from_str(text.as_str()).unwrap()
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
fn ping_frame(frame: &WsMessage) -> PingEvent {
    let WsMessage::Text(text) = frame else {
        panic!("expected a text frame, got {frame:?}");
    };

    #[derive(Deserialize)]
    struct WsMessagePing {
        r#type: String,
        id: String,
    }
    let json: WsMessagePing = serde_json::from_str(text.as_str()).unwrap();
    match json.r#type.as_str() {
        "ping" => PingEvent { id: json.id },
        _ => panic!("expected a ping response, got: {text}"),
    }
}

/// Runs [`forward_egress`] over the given hub messages and returns the frames it sent.
async fn forwarded(messages: Vec<HubMessage>) -> Vec<WsMessage> {
    let (tx, rx) = mpsc::channel(messages.len().max(1));
    for message in messages {
        tx.try_send(message).unwrap();
    }
    drop(tx);
    let (frames_tx, frames_rx) = socket::unbounded();
    forward_egress(rx, frames_tx, Uuid::nil(), Uuid::nil(), SocketTimeouts::default()).await;
    frames_rx.collect().await
}

#[test]
async fn a_close_notice_ends_the_socket_with_its_reason() {
    let frames = forwarded(vec![
        TestMessage::new_ping("1-0"),
        HubMessage::Hub(HubEvent::Shutdown),
        dropped_event(DisconnectReason::Replaced),
        TestMessage::new_ping("2-0"),
    ])
    .await;

    // Hub events are not forwarded, and nothing after the notice is.
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(ping_frame(&frames[0]), PingEvent { id: "1-0".into() });
    assert_eq!(frames[1], close(CLOSE_CODE_DROPPED, "replaced"));
}

#[test]
async fn the_drops_of_other_connections_do_not_end_the_socket() {
    let other = HubMessage::Hub(HubEvent::UserDisconnected {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
        reason: DisconnectReason::SessionInvalid,
    });
    let frames = forwarded(vec![other, TestMessage::new_ping("1-0")]).await;

    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(ping_frame(&frames[0]), PingEvent { id: "1-0".into() });
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
        (DisconnectReason::RateLimited, close_code::POLICY),
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
    let frames = forwarded(vec![TestMessage::new_ping("1-0")]).await;
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(ping_frame(&frames[0]), PingEvent { id: "1-0".into() });
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
        Self::open_with(test, user_id, SocketTimeouts::default(), SocketLimits::default()).await
    }

    async fn open_limited(test: &mut TestHub, user_id: Uuid, limits: SocketLimits) -> Self {
        Self::open_with(test, user_id, SocketTimeouts::default(), limits).await
    }

    async fn open_with(test: &mut TestHub, user_id: Uuid, timeouts: SocketTimeouts, limits: SocketLimits) -> Self {
        let (to_server, server_rx) = socket::unbounded();
        let (server_tx, from_server) = socket::unbounded();
        let server = tokio::spawn(serve_socket(
            server_tx,
            server_rx,
            user_id,
            TestSession::new_random_key(),
            test.hub.clone(),
            SpanContext::empty_context(),
            timeouts,
            limits,
        ));
        let seen = test
            .wait_event(
                None,
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
        let connected = text_json(&client.next_frame().await);
        assert_eq!(connected, serde_json::json!({"type": "connected", "userId": user_id}));
        client
    }

    fn send(&self, text: &str) {
        self.send_frame(WsMessage::Text(text.into()));
    }

    fn send_frame(&self, frame: WsMessage) {
        self.to_server.as_ref().unwrap().unbounded_send(Ok(frame)).unwrap();
    }

    /// The client goes away: its side of the socket ends.
    fn leave(&mut self) {
        self.to_server = None;
    }

    /// Waits until the server side has finished, and returns every frame it sent.
    async fn finished(self) -> Vec<WsMessage> {
        timeout(WAIT_TIMEOUT, self.server)
            .await
            .expect("the server side did not finish")
            .unwrap();
        self.from_server.collect().await
    }

    async fn next_frame(&mut self) -> WsMessage {
        timeout(WAIT_TIMEOUT, self.from_server.next())
            .await
            .expect("no frame from the server")
            .expect("the socket was closed")
    }
}

/// The connection id of the user's socket, from its connect event.
fn connection_of(events: &HubEvents, user_id: Uuid) -> Uuid {
    match events.lifecycle_of(user_id).as_slice() {
        [.., Lifecycle::Connected(connection_id)] => *connection_id,
        other => panic!("no connection of {user_id}: {other:?}"),
    }
}

#[test(serial = "hub-service")]
async fn a_replaced_socket_is_closed_with_the_reason() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();

    let first = Client::open(&mut test, user).await;
    let mut second = Client::open(&mut test, user).await;

    assert_eq!(first.finished().await, [close(CLOSE_CODE_DROPPED, "replaced")]);
    test.broadcast(TestMessage::new_ping("1-0")).await.unwrap();
    assert_eq!(ping_frame(&second.next_frame().await), PingEvent { id: "1-0".into() });

    second.leave();
    let second_id = second.connection_id;
    assert_eq!(second.finished().await, [WsMessage::Close(None)]);
    let seen = test.wait_all_commands().await;
    assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(second_id)]);
}

#[test(serial = "hub-service")]
async fn a_forced_drop_and_a_shutdown_close_the_sockets_with_their_reason() {
    let mut test = TestHub::default_from_env().await;
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
    test.wait_shutdown_complete().await;

    // Once the hub is gone a new socket cannot register, and is closed right away.
    let (to_server, server_rx) = socket::unbounded::<Result<WsMessage, Infallible>>();
    let (server_tx, from_server) = socket::unbounded();
    serve_socket(
        server_tx,
        server_rx,
        Uuid::new_v4(),
        TestSession::new_random_key(),
        test.hub.clone(),
        SpanContext::empty_context(),
        SocketTimeouts::default(),
        SocketLimits::default(),
    )
    .await;
    drop(to_server);
    assert_eq!(from_server.collect::<Vec<_>>().await, []);
}

#[test(serial = "hub-service")]
async fn a_client_leaving_under_traffic_is_not_counted_as_a_drop() {
    let mut test = TestHub::default_from_env().await;

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
                    hub.send_to_connection(connection_id, TestMessage::new_ping(format!("{seq}-0")))
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
        let seen = test.wait_all_commands().await;
        assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(connection_id)]);
        assert!(seen.drops().is_empty());
    }
    assert_eq!(test.hub.stats().await.connections, 0);
}

#[test(serial = "hub-service")]
async fn client_ping_is_broadcast_when_no_target_is_set() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();
    let mut client = Client::open(&mut test, user).await;

    // Invalid frames are ignored; a ping without a target is broadcast back to every socket.
    client.send("not json");
    client.send(r#"{"type":"unknown"}"#);
    client.send(r#"{"type":"ping","id":"hi"}"#);
    assert_eq!(ping_frame(&client.next_frame().await), PingEvent { id: "hi".into() });

    client.leave();
    assert_eq!(client.finished().await, [WsMessage::Close(None)]);
    let seen = test.wait_all_commands().await;
    assert!(seen.drops().is_empty());
}

// --- Frame budget ---------------------------------------------------------------------------------

/// Limits whose buckets do not refill within a test.
fn limits(frames: u32, subscribes: u32, violation_limit: u32) -> SocketLimits {
    SocketLimits {
        frames: RateLimit::new(frames, 0.001),
        subscribes: RateLimit::new(subscribes, 0.001),
        violation_limit,
    }
}

fn command_frame(command_id: Uuid) -> String {
    serde_json::json!({
        "type": "streamCommand", "streamId": "chat:s", "commandId": command_id, "command": "AA=="
    })
    .to_string()
}

fn nack(command_id: Uuid, reason: &str) -> serde_json::Value {
    serde_json::json!({"type": "streamNack", "streamId": "chat:s", "commandId": command_id, "reason": reason})
}

#[test(serial = "hub-service")]
async fn a_denied_command_is_nacked_rate_limited_with_its_command_id() {
    let mut test = TestHub::default_from_env().await;
    let mut client = Client::open_limited(&mut test, Uuid::new_v4(), limits(1, 10, 20)).await;
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());

    // The hub has no streams, so the allowed command is refused for it; the second is over the budget.
    client.send(&command_frame(first));
    client.send(&command_frame(second));

    assert_eq!(text_json(&client.next_frame().await), nack(first, "notSubscribed"));
    assert_eq!(text_json(&client.next_frame().await), nack(second, "rateLimited"));
}

#[test(serial = "hub-service")]
async fn a_denied_subscribe_fails_rate_limited() {
    let mut test = TestHub::default_from_env().await;
    let mut client = Client::open_limited(&mut test, Uuid::new_v4(), limits(10, 1, 20)).await;
    let subscribe = r#"{"type":"streamSubscribe","streamId":"chat:s"}"#;

    client.send(subscribe);
    client.send(subscribe);

    let failed =
        |reason: &str| serde_json::json!({"type": "streamSubscribeFailed", "streamId": "chat:s", "reason": reason});
    assert_eq!(text_json(&client.next_frame().await), failed("failed"));
    assert_eq!(text_json(&client.next_frame().await), failed("rateLimited"));
}

#[test(serial = "hub-service")]
async fn a_flood_closes_the_socket_with_policy_violation_and_a_neighbour_is_untouched() {
    let mut test = TestHub::default_from_env().await;
    let mut neighbour = Client::open(&mut test, Uuid::new_v4()).await;
    let flooder = Client::open_limited(&mut test, Uuid::new_v4(), limits(2, 10, 3)).await;
    let flooder_user = flooder.connection_id;

    // Two frames pass, the next three are denied: the third denial ends the socket.
    for _ in 0..5 {
        flooder.send(r#"{"type":"unknown"}"#);
    }
    assert_eq!(flooder.finished().await, [close(close_code::POLICY, "rate_limited")]);

    let seen = test.wait_all_commands().await;
    assert_eq!(seen.drops(), BTreeMap::from([(DisconnectReason::RateLimited, 1)]));
    assert_eq!(test.hub.stats().await.connections, 1, "{flooder_user}");
    test.broadcast(TestMessage::new_ping("1-0")).await.unwrap();
    assert_eq!(
        ping_frame(&neighbour.next_frame().await),
        PingEvent { id: "1-0".into() }
    );
}

#[test(serial = "hub-service")]
async fn a_flood_of_unparseable_and_binary_frames_closes_without_answers() {
    let mut test = TestHub::default_from_env().await;
    let client = Client::open_limited(&mut test, Uuid::new_v4(), limits(1, 10, 3)).await;

    client.send("not json");
    client.send_frame(WsMessage::Binary(Default::default()));
    client.send("not json");
    client.send_frame(WsMessage::Binary(Default::default()));

    assert_eq!(client.finished().await, [close(close_code::POLICY, "rate_limited")]);
}

#[test(serial = "hub-service")]
async fn ping_and_pong_frames_are_not_charged() {
    let mut test = TestHub::default_from_env().await;
    let mut client = Client::open_limited(&mut test, Uuid::new_v4(), limits(1, 10, 2)).await;
    let command_id = Uuid::new_v4();

    for _ in 0..10 {
        client.send_frame(WsMessage::Ping(Default::default()));
        client.send_frame(WsMessage::Pong(Default::default()));
    }
    // The one frame token is still there.
    client.send(&command_frame(command_id));
    assert_eq!(text_json(&client.next_frame().await), nack(command_id, "notSubscribed"));
    assert!(!client.server.is_finished());
}

#[test(serial = "hub-service")]
async fn frames_that_are_not_valid_requests_are_ignored_without_an_answer() {
    let mut test = TestHub::default_from_env().await;
    let mut client = Client::open(&mut test, Uuid::new_v4()).await;
    let command = |stream_id: &str, command: &str| {
        serde_json::json!({
            "type": "streamCommand", "streamId": stream_id, "commandId": Uuid::new_v4(), "command": command
        })
        .to_string()
    };
    let over_the_size_of_a_command = STANDARD.encode(vec![0u8; MAX_COMMAND_BYTES + 1]);

    for frame in [
        serde_json::json!({"type": "streamSubscribe", "streamId": "has spaces"}).to_string(),
        serde_json::json!({"type": "streamSubscribe", "streamId": ""}).to_string(),
        serde_json::json!({"type": "streamUnsubscribe", "streamId": "has spaces"}).to_string(),
        serde_json::json!({"type": "streamSubscribe"}).to_string(),
        serde_json::json!({"type": "streamCommand", "streamId": "chat:s"}).to_string(),
        serde_json::json!({"type": "unknown"}).to_string(),
        command("has spaces", "AA=="),
        command("chat:s", "!!not base64!!"),
        command("chat:s", &over_the_size_of_a_command),
    ] {
        client.send(&frame);
    }
    client.send_frame(WsMessage::Binary(Default::default()));
    client.send(r#"{"type":"ping","id":"after"}"#);

    // The frames of a socket are handled in order: an answer to one of the above would come before this.
    assert_eq!(ping_frame(&client.next_frame().await), PingEvent { id: "after".into() });
    assert!(!client.server.is_finished(), "the socket was ended");
}

#[test(serial = "hub-service")]
async fn a_command_the_kind_does_not_accept_is_ignored_without_an_answer() {
    let mut test = TestHub::from_env(TestHubOptions {
        with_streams: Some(StreamConfig::test_default()),
        ..TestHubOptions::default()
    })
    .await;
    let mut client = Client::open(&mut test, Uuid::new_v4()).await;
    // A stream of its own: the log is shared between the runs of the tests.
    let stream_id = TestStreamId::new_pixel_map().to_string();
    let paint = |x: u8, y: u8, color: u8| {
        serde_json::json!({
            "type": "streamCommand", "streamId": stream_id, "commandId": Uuid::new_v4(),
            "command": STANDARD.encode(TestMessage::new_paint_command(x, y, color))
        })
        .to_string()
    };
    client.send(&serde_json::json!({"type": "streamSubscribe", "streamId": stream_id}).to_string());
    assert_eq!(text_json(&client.next_frame().await)["type"], "streamSync");

    // A paint off the map and one with a color outside the palette, then a good one.
    client.send(&paint(99, 0, 1));
    client.send(&paint(0, 0, 200));
    client.send(&paint(1, 1, 1));

    // The ignored commands used no version, and no answer came before the delivery of the good one.
    let delivered = text_json(&client.next_frame().await);
    assert_eq!(delivered["type"], "streamCommands");
    assert_eq!(delivered["commands"][0]["version"], 1);
}

// --- Stuck and silent peers ----------------------------------------------------------------------

/// Short limits, so a stuck socket ends in a blink.
fn quick_timeouts() -> SocketTimeouts {
    SocketTimeouts {
        write: Duration::from_millis(100),
        ping_interval: Duration::from_millis(40),
        idle: Duration::from_millis(250),
        send_drain: Duration::from_secs(1),
    }
}

/// A socket of a peer that does not read: no write ever completes.
struct StuckSink;

impl Sink<WsMessage> for StuckSink {
    type Error = Infallible;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Pending
    }

    fn start_send(self: std::pin::Pin<&mut Self>, _: WsMessage) -> Result<(), Infallible> {
        Ok(())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Pending
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Infallible>> {
        std::task::Poll::Pending
    }
}

#[test]
async fn a_peer_that_does_not_read_ends_the_egress_at_the_write_timeout() {
    let (tx, rx) = mpsc::channel(4);
    tx.try_send(TestMessage::new_ping("1-0")).unwrap();

    // The hub still holds its sender: only the write timeout can end the task.
    timeout(
        WAIT_TIMEOUT,
        forward_egress(rx, StuckSink, Uuid::nil(), Uuid::nil(), quick_timeouts()),
    )
    .await
    .expect("a write that never completes kept the egress alive");
    drop(tx);
}

#[test]
async fn a_peer_that_does_not_read_the_pings_ends_the_egress_too() {
    let (tx, rx) = mpsc::channel(4);

    timeout(
        WAIT_TIMEOUT,
        forward_egress(rx, StuckSink, Uuid::nil(), Uuid::nil(), quick_timeouts()),
    )
    .await
    .expect("an unanswered ping write kept the egress alive");
    drop(tx);
}

#[test(serial = "hub-service")]
async fn a_silent_peer_is_pinged_and_then_closed_as_idle() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();
    let client = Client::open_with(&mut test, user, quick_timeouts(), SocketLimits::default()).await;
    let connection_id = client.connection_id;

    let frames = client.finished().await;
    let (close, pings) = frames.split_last().expect("no frames");
    assert_eq!(close, &WsMessage::Close(None), "{frames:?}");
    assert!(!pings.is_empty(), "no ping before the socket was closed: {frames:?}");
    assert!(
        pings.iter().all(|frame| matches!(frame, WsMessage::Ping(_))),
        "{frames:?}"
    );

    let seen = test.wait_all_commands().await;
    assert_eq!(seen.lifecycle_of(user), [Lifecycle::Disconnected(connection_id)]);
    assert!(seen.drops().is_empty());
}

#[test(serial = "hub-service")]
async fn a_peer_that_answers_the_pings_is_not_idle() {
    let mut test = TestHub::default_from_env().await;
    let user = Uuid::new_v4();
    let mut client = Client::open_with(&mut test, user, quick_timeouts(), SocketLimits::default()).await;

    // Three times the idle limit, with a pong every 40 ms.
    for _ in 0..19 {
        client
            .to_server
            .as_ref()
            .unwrap()
            .unbounded_send(Ok(WsMessage::Pong(Default::default())))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    assert!(!client.server.is_finished(), "a live peer was closed as idle");

    client.leave();
    let frames = client.finished().await;
    assert_eq!(frames.last(), Some(&WsMessage::Close(None)), "{frames:?}");
}
