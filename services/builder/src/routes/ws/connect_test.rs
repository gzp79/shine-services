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
        }],
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
        chat_payload("1-0", "hello"),
        HubMessage::Hub(HubEvent::Shutdown),
        dropped_event(DisconnectReason::Replaced),
        chat_payload("2-0", "after the close"),
    ])
    .await;

    // Hub events are not forwarded, and nothing after the notice is.
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(chat_frame(&frames[0]), [("1-0".to_string(), "hello".to_string())]);
    assert_eq!(frames[1], close(CLOSE_CODE_DROPPED, "replaced"));
}

#[test]
async fn the_drops_of_other_connections_do_not_end_the_socket() {
    let other = HubMessage::Hub(HubEvent::UserDisconnected {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
        reason: DisconnectReason::SessionInvalid,
    });
    let frames = forwarded(vec![other, chat_payload("1-0", "still open")]).await;

    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(chat_frame(&frames[0]), [("1-0".to_string(), "still open".to_string())]);
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
    let frames = forwarded(vec![chat_payload("1-0", "buffered")]).await;
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(chat_frame(&frames[0]), [("1-0".to_string(), "buffered".to_string())]);
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
        Self::open_with(test, chat, user_id, None).await
    }

    async fn open_with(
        test: &mut TestHub,
        chat: &ChatService,
        user_id: Uuid,
        streams: Option<CommandStreamService>,
    ) -> Self {
        let (to_server, server_rx) = socket::unbounded();
        let (server_tx, from_server) = socket::unbounded();
        let server = tokio::spawn(serve_socket(
            server_tx,
            server_rx,
            user_id,
            key(),
            test.hub.clone(),
            chat.clone(),
            streams,
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

    assert_eq!(first.finished().await, [close(CLOSE_CODE_DROPPED, "replaced")]);
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
        chat.clone(),
        None,
        SpanContext::empty_context(),
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

    // Invalid frames are ignored; the valid chat goes through the room and back to every socket.
    client.send("not json");
    client.send(r#"{"type":"unknown"}"#);
    client.send(r#"{"type":"chat","text":"hi"}"#);
    let delivered = chat_frame(&client.next_frame().await);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].1, "hi");

    client.leave();
    assert_eq!(client.finished().await, [WsMessage::Close(None)]);
    let seen = barrier(&test.hub, &mut test.events).await;
    assert_eq!(drops(&seen), dropped(&[]));
}

// --- Streams on the socket ----------------------------------------------------------------------

use crate::services::command_stream::{
    test_utils::{add, fast_cadence, setup_with, unique_id, Test},
    AllowAll,
};
use serde_json::{json, Value};
use shine_game::replay::protocol::ClientFrame;
use shine_infra::db::command_log::StreamId;
use std::sync::Arc;

async fn stream_hub() -> Option<(TestHub, ChatService, Test)> {
    let streams = setup_with(Arc::new(AllowAll), |config| {
        fast_cadence(config);
        config.unload_grace_seconds = 60;
    })
    .await?;
    let test = create_test_hub(TestHubOptions {
        command_streams: Some(streams.service.clone()),
        ..Default::default()
    })
    .await?;
    let chat_db = RedisChatCommentsDb::new(&test.redis, 200, 3600).await.unwrap();
    let chat = ChatService::with_room(chat_db, test.chat_room.clone());
    Some((test, chat, streams))
}

fn json_of(frame: &WsMessage) -> Value {
    let WsMessage::Text(text) = frame else {
        panic!("expected a text frame, got {frame:?}");
    };
    serde_json::from_str(text.as_str()).unwrap()
}

impl Client {
    /// The next stream frame that carries state or commands, skipping the others.
    async fn next_frame_of_stream(&mut self) -> Value {
        loop {
            let frame = json_of(&self.next_frame().await);
            if matches!(frame["type"].as_str(), Some("streamSync" | "streamCommands")) {
                return frame;
            }
        }
    }

    /// The next frame of the given type, skipping the others.
    async fn next_of(&mut self, ty: &str) -> Value {
        loop {
            let frame = json_of(&self.next_frame().await);
            if frame["type"] == ty {
                return frame;
            }
        }
    }

    fn subscribe(&self, stream_id: &StreamId, kind: &str, resume: Option<(i64, i64)>) {
        let mut frame = json!({ "type": "streamSubscribe", "streamId": stream_id.as_str(), "kind": kind });
        if let Some((generation, version)) = resume {
            frame["resume"] = json!({ "generation": generation, "version": version });
        }
        self.send(&frame.to_string());
    }

    fn write(&self, stream_id: &StreamId, command_id: Uuid, text: &str) {
        self.send(
            &json!({
                "type": "streamCommand", "streamId": stream_id.as_str(), "commandId": command_id, "command": add(text)
            })
            .to_string(),
        );
    }
}

fn versions(frame: &Value) -> Vec<i64> {
    frame["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["version"].as_i64().unwrap())
        .collect()
}

#[test]
async fn a_resume_frame_is_parsed() {
    let frame =
        r#"{"type":"streamSubscribe","streamId":"chat-global","kind":"chat","resume":{"generation":2,"version":7}}"#;
    let Some(Incoming::Stream(ClientFrame::StreamSubscribe { resume, .. })) = Incoming::parse(frame) else {
        panic!("not a subscribe");
    };
    assert_eq!(resume.map(|r| (r.generation, r.version)), Some((2, 7)));
    assert!(matches!(
        Incoming::parse(r#"{"type":"chat","text":"hi"}"#),
        Some(Incoming::Chat(_))
    ));
    assert!(Incoming::parse(r#"{"type":"nonsense"}"#).is_none());
}

#[test(serial = "hub-service")]
async fn a_subscriber_gets_a_sync_and_sees_its_own_command() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (user, stream_id) = (Uuid::new_v4(), unique_id());
    let mut client = Client::open_with(&mut test, &chat, user, Some(streams.service.clone())).await;

    client.subscribe(&stream_id, "chat", None);
    let sync = client.next_of("streamSync").await;
    assert_eq!(sync["streamId"], stream_id.as_str());
    assert_eq!(sync["snapshot"], Value::Null);
    assert_eq!(sync["snapshotEvery"], 5);

    let command_id = Uuid::new_v4();
    client.write(&stream_id, command_id, "hello");
    let commands = client.next_of("streamCommands").await;
    assert_eq!(versions(&commands), [1]);
    assert_eq!(commands["commands"][0]["commandId"], command_id.to_string());
    assert_eq!(commands["commands"][0]["userId"], user.to_string());
    assert_eq!(commands["commands"][0]["command"], r#"{"type":"add","text":"hello"}"#);

    // The same command id again is refused, not applied twice.
    client.write(&stream_id, command_id, "hello");
    let nack = client.next_of("streamNack").await;
    assert_eq!(nack["reason"], "duplicate");
    assert_eq!(nack["commandId"], command_id.to_string());

    client.leave();
    client.finished().await;
}

#[test(serial = "hub-service")]
async fn a_command_for_an_unsubscribed_stream_gets_not_subscribed() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (user, stream_id) = (Uuid::new_v4(), unique_id());
    let mut client = Client::open_with(&mut test, &chat, user, Some(streams.service.clone())).await;

    client.write(&stream_id, Uuid::new_v4(), "nobody listens");
    assert_eq!(client.next_of("streamNack").await["reason"], "notSubscribed");

    client.subscribe(&stream_id, "chat", None);
    client.next_of("streamSync").await;
    client.send(&json!({ "type": "streamUnsubscribe", "streamId": stream_id.as_str() }).to_string());
    client.write(&stream_id, Uuid::new_v4(), "after the unsubscribe");
    assert_eq!(client.next_of("streamNack").await["reason"], "notSubscribed");
}

#[test(serial = "hub-service")]
async fn invalid_and_oversized_commands_get_invalid() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (user, stream_id) = (Uuid::new_v4(), unique_id());
    let mut client = Client::open_with(&mut test, &chat, user, Some(streams.service.clone())).await;
    client.subscribe(&stream_id, "chat", None);
    client.next_of("streamSync").await;

    client.write(&stream_id, Uuid::new_v4(), &"x".repeat(300));
    assert_eq!(client.next_of("streamNack").await["reason"], "invalid");
    client.send(
        &json!({ "type": "streamCommand", "streamId": stream_id.as_str(), "commandId": Uuid::new_v4(), "command": "nope" })
            .to_string(),
    );
    assert_eq!(client.next_of("streamNack").await["reason"], "invalid");
}

#[test(serial = "hub-service")]
async fn an_unknown_kind_gets_subscribe_failed_unknown_kind() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (user, stream_id) = (Uuid::new_v4(), unique_id());
    let mut client = Client::open_with(&mut test, &chat, user, Some(streams.service.clone())).await;

    client.subscribe(&stream_id, "nonsense", None);
    let failed = client.next_of("streamSubscribeFailed").await;
    assert_eq!(failed["streamId"], stream_id.as_str());
    assert_eq!(failed["reason"], "unknownKind");

    // A stream keeps the kind it was created with.
    client.subscribe(&stream_id, "chat", None);
    client.next_of("streamSync").await;
    client.subscribe(&stream_id, "counter", None);
    assert_eq!(client.next_of("streamSubscribeFailed").await["reason"], "kindMismatch");
}

#[test(serial = "hub-service")]
async fn without_command_streams_a_subscribe_is_unavailable() {
    let Some((mut test, chat)) = socket_hub().await else {
        return;
    };
    let mut client = Client::open(&mut test, &chat, Uuid::new_v4()).await;
    client.subscribe(&unique_id(), "chat", None);
    assert_eq!(client.next_of("streamSubscribeFailed").await["reason"], "unavailable");
}

/// A stream with a first subscriber and `count` commands written, so its worker is resident.
async fn resident_stream(
    test: &mut TestHub,
    chat: &ChatService,
    streams: CommandStreamService,
    count: i64,
) -> (StreamId, Client, i64) {
    let (user, stream_id) = (Uuid::new_v4(), unique_id());
    let mut first = Client::open_with(test, chat, user, Some(streams)).await;
    first.subscribe(&stream_id, "chat", None);
    let generation = first.next_of("streamSync").await["generation"].as_i64().unwrap();
    for i in 1..=count {
        first.write(&stream_id, Uuid::new_v4(), &format!("m{i}"));
    }
    let mut seen = 0;
    while seen < count {
        seen += versions(&first.next_of("streamCommands").await).len() as i64;
    }
    (stream_id, first, generation)
}

#[test(serial = "hub-service")]
async fn resume_inside_the_window_sends_only_the_tail() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (stream_id, _first, generation) = resident_stream(&mut test, &chat, streams.service.clone(), 3).await;

    let mut second = Client::open_with(&mut test, &chat, Uuid::new_v4(), Some(streams.service.clone())).await;
    second.subscribe(&stream_id, "chat", Some((generation, 2)));
    let frame = second.next_frame_of_stream().await;
    assert_eq!(frame["type"], "streamCommands");
    assert_eq!(versions(&frame), [3]);

    // Resuming at the head sends nothing: the next frame is the next command.
    second.subscribe(&stream_id, "chat", Some((generation, 3)));
    second.write(&stream_id, Uuid::new_v4(), "m4");
    let frame = second.next_frame_of_stream().await;
    assert_eq!(frame["type"], "streamCommands");
    assert_eq!(versions(&frame), [4]);
}

#[test(serial = "hub-service")]
async fn resume_with_a_changed_generation_sends_a_full_sync() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (stream_id, _first, generation) = resident_stream(&mut test, &chat, streams.service.clone(), 3).await;

    let mut second = Client::open_with(&mut test, &chat, Uuid::new_v4(), Some(streams.service.clone())).await;
    second.subscribe(&stream_id, "chat", Some((generation + 1, 2)));
    let sync = second.next_frame_of_stream().await;
    assert_eq!(sync["type"], "streamSync");
    assert_eq!(versions(&sync), [1, 2, 3]);
}

#[test(serial = "hub-service")]
async fn resume_above_the_head_sends_a_full_sync() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    let (stream_id, _first, generation) = resident_stream(&mut test, &chat, streams.service.clone(), 3).await;

    let mut second = Client::open_with(&mut test, &chat, Uuid::new_v4(), Some(streams.service.clone())).await;
    second.subscribe(&stream_id, "chat", Some((generation, 99)));
    let sync = second.next_frame_of_stream().await;
    assert_eq!(sync["type"], "streamSync");
    assert_eq!(versions(&sync), [1, 2, 3]);
}

#[test(serial = "hub-service")]
async fn resume_older_than_the_base_version_sends_a_full_sync() {
    let Some((mut test, chat, streams)) = stream_hub().await else {
        return;
    };
    // Six commands with a snapshot every five: the base of the worker is version 5 once the checkpoint is out.
    let (stream_id, mut first, generation) = resident_stream(&mut test, &chat, streams.service.clone(), 6).await;
    assert_eq!(first.next_of("streamCheckpoint").await["version"], 5);

    let mut second = Client::open_with(&mut test, &chat, Uuid::new_v4(), Some(streams.service.clone())).await;
    second.subscribe(&stream_id, "chat", Some((generation, 2)));
    let sync = second.next_frame_of_stream().await;
    assert_eq!(sync["type"], "streamSync");
    assert_eq!(sync["snapshot"]["version"], 5);
    assert_eq!(versions(&sync), [6]);
}
