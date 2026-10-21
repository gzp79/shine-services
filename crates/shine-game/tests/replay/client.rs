use shine_game::replay::{
    encode_command, encode_snapshot,
    protocol::{encode_snapshot_data, hash_to_hex, ClientFrame, ServerFrame, WireCommand, WireResume, WireSnapshot},
    ChatCommand, ChatState, ClientEvent, ClientStream, CommandEnvelope, Replica, SyncStatus,
};
use shine_test::test;
use uuid::Uuid;

const STREAM: &str = "chat-global";
const EVERY: i64 = 5;

fn me() -> Uuid {
    Uuid::from_u128(1)
}

fn other() -> Uuid {
    Uuid::from_u128(2)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(1000 + n)
}

fn add(text: &str) -> ChatCommand {
    ChatCommand::Add { text: text.to_string() }
}

/// The server's log and the state it replays.
struct Server {
    replica: Replica<ChatState>,
    log: Vec<WireCommand>,
}

impl Server {
    fn new() -> Self {
        Self {
            replica: Replica::new(),
            log: vec![],
        }
    }

    /// Orders a command at the next version.
    fn push(&mut self, user_id: Uuid, command_id: Uuid, command: &ChatCommand) -> WireCommand {
        let wire = WireCommand {
            version: self.replica.version() + 1,
            command_id,
            user_id,
            command: encode_command::<ChatState>(command).unwrap(),
        };
        self.replica
            .apply_next(&CommandEnvelope {
                version: wire.version,
                command_id,
                user_id,
                command: command.clone(),
            })
            .unwrap();
        self.log.push(wire.clone());
        wire
    }

    fn snapshot(&self) -> WireSnapshot {
        WireSnapshot {
            version: self.replica.version(),
            hash: hash_to_hex(self.replica.hash()),
            data: encode_snapshot_data(&encode_snapshot(self.replica.state()).unwrap()),
        }
    }

    /// The full state as a sync, without a snapshot.
    fn sync(&self, generation: i64) -> ServerFrame {
        sync(generation, None, self.log.clone())
    }
}

fn sync(generation: i64, snapshot: Option<WireSnapshot>, commands: Vec<WireCommand>) -> ServerFrame {
    ServerFrame::StreamSync {
        stream_id: STREAM.into(),
        generation,
        snapshot_every: EVERY,
        snapshot,
        commands,
    }
}

fn commands(generation: i64, commands: Vec<WireCommand>) -> ServerFrame {
    ServerFrame::StreamCommands {
        stream_id: STREAM.into(),
        generation,
        commands,
    }
}

fn nack(command_id: Uuid, reason: &str) -> ServerFrame {
    ServerFrame::StreamNack {
        stream_id: STREAM.into(),
        command_id,
        reason: reason.into(),
    }
}

fn texts(state: &ChatState) -> Vec<&str> {
    state.messages.iter().map(|m| m.text.as_str()).collect()
}

/// A client that is live on an empty stream of generation 1.
fn live() -> ClientStream<ChatState> {
    let mut client = ClientStream::<ChatState>::new(STREAM, me());
    client.on_connected();
    client.on_frame(sync(1, None, vec![]));
    assert_eq!(client.status(), &SyncStatus::Live);
    client
}

fn subscribe_resume(frame: &ClientFrame) -> Option<WireResume> {
    match frame {
        ClientFrame::StreamSubscribe { resume, .. } => *resume,
        other => panic!("expected a subscribe, got {other:?}"),
    }
}

#[test]
fn a_sync_then_commands_reach_the_state_of_the_server() {
    let mut server = Server::new();
    for i in 1..=5 {
        server.push(other(), id(i), &add(&format!("m{i}")));
    }
    let snapshot = server.snapshot();
    let tail = vec![server.push(other(), id(6), &add("m6"))];

    let mut client = ClientStream::<ChatState>::new(STREAM, me());
    assert_eq!(client.status(), &SyncStatus::Connecting);
    let subscribe = client.on_connected();
    assert_eq!(client.status(), &SyncStatus::Syncing);
    assert_eq!(
        subscribe_resume(&subscribe[0]),
        None,
        "nothing to resume on a first connect"
    );

    let outcome = client.on_frame(sync(1, Some(snapshot), tail));
    assert_eq!(outcome.event, ClientEvent::Synced);
    assert_eq!(client.status(), &SyncStatus::Live);

    let more = vec![
        server.push(other(), id(7), &add("m7")),
        server.push(other(), id(8), &add("m8")),
    ];
    assert_eq!(
        client.on_frame(commands(1, more)).event,
        ClientEvent::Applied { commands: 2 }
    );
    assert_eq!(client.authoritative().version(), 8);
    assert_eq!(client.local(), server.replica.state());
}

#[test]
fn a_gap_asks_for_a_fresh_sync() {
    let mut server = Server::new();
    let first = server.push(other(), id(1), &add("a"));
    server.push(other(), id(2), &add("lost"));
    let third = server.push(other(), id(3), &add("c"));
    let mut client = live();
    client.on_frame(commands(1, vec![first]));

    let outcome = client.on_frame(commands(1, vec![third]));
    assert_eq!(outcome.event, ClientEvent::ResyncRequested("a command is missing"));
    assert_eq!(client.status(), &SyncStatus::Resyncing);
    assert_eq!(outcome.send.len(), 1);
    assert_eq!(subscribe_resume(&outcome.send[0]), None, "a resync starts from scratch");
    assert_eq!(texts(client.local()), ["a"], "what was applied stays");
}

#[test]
fn a_reset_then_a_sync_rebuilds_the_state() {
    let mut server = Server::new();
    let old = server.push(other(), id(1), &add("old"));
    let mut client = live();
    client.on_frame(commands(1, vec![old]));

    assert_eq!(
        client
            .on_frame(ServerFrame::StreamReset { stream_id: STREAM.into() })
            .event,
        ClientEvent::Reset
    );
    assert_eq!(client.status(), &SyncStatus::Resyncing);

    // Frames of the old subscription still in flight do not touch the state.
    let stale = server.push(other(), id(2), &add("stale"));
    assert_eq!(client.on_frame(commands(1, vec![stale])).event, ClientEvent::None);
    assert_eq!(texts(client.local()), ["old"]);

    let mut rebuilt = Server::new();
    rebuilt.push(other(), id(10), &add("new history"));
    assert_eq!(client.on_frame(rebuilt.sync(2)).event, ClientEvent::Synced);
    assert_eq!(client.status(), &SyncStatus::Live);
    assert_eq!(texts(client.local()), ["new history"]);
}

#[test]
fn pending_writes_survive_a_resync_and_are_sent_again() {
    let mut client = live();
    let sent = client.submit(id(50), add("mine"));
    assert_eq!(
        sent,
        vec![ClientFrame::StreamCommand {
            stream_id: STREAM.into(),
            command_id: id(50),
            command: r#"{"type":"add","text":"mine"}"#.into()
        }]
    );
    assert_eq!(texts(client.local()), ["mine"], "shown at once");
    assert!(client.authoritative().state().messages.is_empty());

    // A new history that does not contain the write: it goes out again with the same id, on top of the sync.
    let mut server = Server::new();
    server.push(other(), id(1), &add("theirs"));
    let outcome = client.on_frame(server.sync(2));
    assert_eq!(outcome.send, sent);
    assert_eq!(client.pending().len(), 1);
    assert_eq!(texts(client.local()), ["theirs", "mine"]);

    // Once the log has it, it is confirmed and the view is the authoritative state.
    let landed = server.push(me(), id(50), &add("mine"));
    client.on_frame(commands(2, vec![landed]));
    assert!(client.pending().is_empty());
    assert_eq!(client.local(), client.authoritative().state());
    assert_eq!(texts(client.local()), ["theirs", "mine"]);
}

#[test]
fn a_sync_that_holds_a_pending_write_confirms_it() {
    let mut client = live();
    client.submit(id(50), add("mine"));
    let mut server = Server::new();
    let landed = server.push(me(), id(50), &add("mine"));

    let outcome = client.on_frame(sync(1, None, vec![landed]));
    assert!(outcome.send.is_empty(), "not sent again");
    assert!(client.pending().is_empty());
    assert_eq!(texts(client.local()), ["mine"]);
}

#[test]
fn a_duplicate_nack_drops_a_pending_write_and_transient_ones_keep_it() {
    let mut client = live();
    client.submit(id(1), add("a"));
    client.submit(id(2), add("b"));
    client.submit(id(3), add("c"));
    client.submit(id(4), add("d"));

    // Already in the log, or refused for good: gone from the pending list and from the view.
    let outcome = client.on_frame(nack(id(1), "duplicate"));
    assert_eq!(
        outcome.event,
        ClientEvent::Nacked {
            command_id: id(1),
            reason: "duplicate".into(),
            dropped: true
        }
    );
    client.on_frame(nack(id(4), "invalid"));
    assert_eq!(texts(client.local()), ["b", "c"]);

    // Transient: stays, and goes out again on request.
    for reason in ["failed", "busy", "notReady"] {
        let outcome = client.on_frame(nack(id(2), reason));
        assert_eq!(
            outcome.event,
            ClientEvent::Nacked {
                command_id: id(2),
                reason: reason.into(),
                dropped: false
            }
        );
    }
    assert_eq!(client.pending().len(), 2);
    let resent = client.retry_pending();
    assert_eq!(resent.len(), 2);

    // A nack for a write nobody knows is ignored.
    assert_eq!(client.on_frame(nack(id(99), "duplicate")).event, ClientEvent::None);
}

#[test]
fn a_checkpoint_that_differs_asks_for_a_fresh_sync() {
    let mut server = Server::new();
    let mut client = live();
    for i in 1..=7 {
        let wire = server.push(other(), id(i), &add(&format!("m{i}")));
        client.on_frame(commands(1, vec![wire]));
    }
    let checkpoint = |version: i64, hash: u64| ServerFrame::StreamCheckpoint {
        stream_id: STREAM.into(),
        version,
        hash: hash_to_hex(hash),
    };

    // The hash at 5 was taken when the client passed it, though two commands came after it.
    let mut at_five = Replica::<ChatState>::new();
    for wire in &server.log[..5] {
        at_five
            .apply_next(&CommandEnvelope {
                version: wire.version,
                command_id: wire.command_id,
                user_id: wire.user_id,
                command: serde_json::from_str(&wire.command).unwrap(),
            })
            .unwrap();
    }
    assert_eq!(client.on_frame(checkpoint(5, at_five.hash())).event, ClientEvent::None);
    assert_eq!(client.status(), &SyncStatus::Live);
    // A version the client did not hash is no evidence.
    assert_eq!(client.on_frame(checkpoint(4, 0)).event, ClientEvent::None);

    let outcome = client.on_frame(checkpoint(5, at_five.hash() ^ 1));
    assert_eq!(
        outcome.event,
        ClientEvent::ResyncRequested("the state differs from the checkpoint")
    );
    assert_eq!(client.status(), &SyncStatus::Resyncing);
    assert_eq!(subscribe_resume(&outcome.send[0]), None);
}

#[test]
fn the_view_is_the_authoritative_state_plus_the_pending_writes() {
    let mut server = Server::new();
    let mut client = live();
    let theirs = server.push(other(), id(1), &add("theirs"));
    client.submit(id(50), add("first"));
    client.submit(id(51), add("second"));
    assert_eq!(texts(client.local()), ["first", "second"]);

    // Their command is ordered before ours: the view has it first, our writes still on top.
    client.on_frame(commands(1, vec![theirs]));
    assert_eq!(texts(client.local()), ["theirs", "first", "second"]);

    let first = server.push(me(), id(50), &add("first"));
    client.on_frame(commands(1, vec![first]));
    assert_eq!(client.pending().len(), 1);
    assert_eq!(texts(client.local()), ["theirs", "first", "second"]);
    assert_eq!(texts(client.authoritative().state()), ["theirs", "first"]);

    // Our edit of our own message also shows at once.
    let message = client.authoritative().state().messages[1].id;
    client.submit(
        id(52),
        ChatCommand::Replace {
            id: message,
            text: "fixed".into(),
        },
    );
    assert_eq!(texts(client.local()), ["theirs", "fixed", "second"]);
    assert!(client.local().messages[1].edited);
}

#[test]
fn after_a_disconnect_the_client_resumes_and_sends_its_pending_writes() {
    let mut server = Server::new();
    let mut client = live();
    for i in 1..=3 {
        let wire = server.push(other(), id(i), &add(&format!("m{i}")));
        client.on_frame(commands(1, vec![wire]));
    }

    client.on_disconnected();
    assert_eq!(client.status(), &SyncStatus::Offline);
    assert!(
        client.submit(id(50), add("offline")).is_empty(),
        "nothing is sent without a socket"
    );
    assert_eq!(texts(client.local()), ["m1", "m2", "m3", "offline"]);

    let subscribe = client.on_connected();
    assert_eq!(client.status(), &SyncStatus::Syncing);
    assert_eq!(
        subscribe_resume(&subscribe[0]),
        Some(WireResume { generation: 1, version: 3 })
    );

    // The server answers a resume at the head with an empty list: the position was accepted.
    let outcome = client.on_frame(commands(1, vec![]));
    assert_eq!(outcome.event, ClientEvent::Synced);
    assert_eq!(client.status(), &SyncStatus::Live);
    assert_eq!(outcome.send.len(), 1, "the offline write goes out");
    assert_eq!(client.pending().len(), 1);
}

#[test]
fn a_resume_answered_with_a_sync_replaces_the_state() {
    let mut server = Server::new();
    let mut client = live();
    let wire = server.push(other(), id(1), &add("old"));
    client.on_frame(commands(1, vec![wire]));
    client.on_disconnected();
    client.on_connected();

    let mut rebuilt = Server::new();
    rebuilt.push(other(), id(9), &add("rebuilt"));
    assert_eq!(client.on_frame(rebuilt.sync(2)).event, ClientEvent::Synced);
    assert_eq!(texts(client.local()), ["rebuilt"]);
    assert_eq!(client.status(), &SyncStatus::Live);
}

#[test]
fn commands_of_another_generation_ask_for_a_fresh_sync() {
    let mut server = Server::new();
    let mut client = live();
    let wire = server.push(other(), id(1), &add("a"));
    let outcome = client.on_frame(commands(2, vec![wire]));
    assert_eq!(outcome.event, ClientEvent::ResyncRequested("the history changed"));
    assert_eq!(client.status(), &SyncStatus::Resyncing);
}

#[test]
fn a_refused_subscribe_fails_the_stream_until_the_next_connection() {
    let mut client = ClientStream::<ChatState>::new(STREAM, me());
    client.on_connected();
    let outcome = client.on_frame(ServerFrame::StreamSubscribeFailed {
        stream_id: STREAM.into(),
        reason: "forbidden".into(),
    });
    assert_eq!(outcome.event, ClientEvent::Failed("forbidden".into()));
    assert_eq!(client.status(), &SyncStatus::Failed("forbidden".into()));

    client.on_disconnected();
    client.on_connected();
    assert_eq!(client.status(), &SyncStatus::Syncing);
}

#[test]
fn a_snapshot_that_does_not_match_its_hash_fails_the_stream() {
    let mut server = Server::new();
    server.push(other(), id(1), &add("a"));
    let mut snapshot = server.snapshot();
    snapshot.hash = hash_to_hex(server.replica.hash() ^ 1);
    let mut client = ClientStream::<ChatState>::new(STREAM, me());
    client.on_connected();
    let outcome = client.on_frame(sync(1, Some(snapshot), vec![]));
    assert_eq!(
        outcome.event,
        ClientEvent::Failed("the snapshot does not match its hash".into())
    );

    let mut client = ClientStream::<ChatState>::new(STREAM, me());
    client.on_connected();
    let broken = WireSnapshot {
        version: 1,
        hash: hash_to_hex(0),
        data: "not base64!".into(),
    };
    assert!(matches!(
        client.on_frame(sync(1, Some(broken), vec![])).event,
        ClientEvent::Failed(_)
    ));
}

#[test]
fn frames_of_another_stream_are_ignored() {
    let mut client = live();
    let outcome = client.on_frame(ServerFrame::StreamReset {
        stream_id: "other-stream".into(),
    });
    assert_eq!(outcome.event, ClientEvent::None);
    assert_eq!(client.status(), &SyncStatus::Live);
}

#[test]
fn a_sync_that_never_arrives_takes_the_client_offline() {
    let mut client = ClientStream::<ChatState>::new(STREAM, me());
    client.on_connected();
    client.on_sync_timeout();
    assert_eq!(client.status(), &SyncStatus::Offline);

    let mut live = live();
    live.on_sync_timeout();
    assert_eq!(
        live.status(),
        &SyncStatus::Live,
        "a live stream has nothing to wait for"
    );
}
