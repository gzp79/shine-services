use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use shine_game::replica::{
    encode_snapshot, ClientStream, CommandEnvelope, PixelMapCommand, PixelMapState, Replica, StreamId, StreamStatus,
    SubmitError, PIXEL_MAP_WIDTH,
};
use shine_test::test;
use uuid::Uuid;

const ME: u128 = 100;
const OTHER: u128 = 200;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn paint(x: u8, y: u8, color: u8) -> PixelMapCommand {
    PixelMapCommand::Paint { x, y, color }
}

fn at(state: &PixelMapState, x: usize, y: usize) -> u8 {
    state.pixels[y * PIXEL_MAP_WIDTH + x]
}

/// The server side of the protocol: a real replica that makes genuine frames.
struct Server {
    replica: Replica<PixelMapState>,
    entries: Vec<Value>,
}

impl Server {
    fn new() -> Self {
        Self {
            replica: Replica::new(),
            entries: Vec::new(),
        }
    }

    fn append(&mut self, command_id: u128, user: u128, command: PixelMapCommand) {
        let envelope = CommandEnvelope::new(
            self.replica.version() + 1,
            id(command_id),
            id(user),
            self.replica.chain_hash(),
            command.clone(),
        );
        self.replica.apply_next(&envelope).unwrap();
        self.entries.push(json!({
            "version": envelope.header.version, "commandId": id(command_id), "userId": id(user),
            "command": serde_json::to_value(&command).unwrap()
        }));
    }

    fn snapshot(&self) -> Value {
        let checkpoint = self.replica.checkpoint();
        json!({
            "version": checkpoint.version,
            "chainHash": format!("{:016x}", checkpoint.chain_hash),
            "stateHash": format!("{:016x}", checkpoint.state_hash),
            "data": STANDARD.encode(encode_snapshot(self.replica.state()).unwrap()),
        })
    }

    fn sync(&self, snapshot: Option<Value>, tail_from: usize) -> String {
        json!({"type": "streamSync", "streamId": "s", "snapshot": snapshot, "commands": self.entries[tail_from..]})
            .to_string()
    }

    fn commands(&self, from: usize) -> String {
        json!({"type": "streamCommands", "streamId": "s", "commands": self.entries[from..]}).to_string()
    }

    fn checkpoint(&self) -> String {
        let checkpoint = self.replica.checkpoint();
        json!({
            "type": "streamCheckpoint", "streamId": "s", "version": checkpoint.version,
            "stateHash": format!("{:016x}", checkpoint.state_hash)
        })
        .to_string()
    }
}

fn client() -> ClientStream<PixelMapState> {
    ClientStream::new(StreamId::parse("s").unwrap(), id(ME))
}

fn live_client(server: &Server) -> ClientStream<PixelMapState> {
    let mut client = client();
    client.handle_message(&server.sync(None, 0));
    assert_eq!(client.status(), StreamStatus::Live);
    client
}

fn nack(command: u128, reason: &str) -> String {
    json!({"type": "streamNack", "streamId": "s", "commandId": id(command), "reason": reason}).to_string()
}

fn frame_type(frame: &str) -> String {
    serde_json::from_str::<Value>(frame).unwrap()["type"]
        .as_str()
        .unwrap()
        .to_string()
}

fn command_id_of(frame: &str) -> Uuid {
    serde_json::from_str::<Value>(frame).unwrap()["commandId"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn the_subscribe_frame_names_the_stream_and_the_kind() {
    let frame: Value = serde_json::from_str(&client().subscribe_frame()).unwrap();

    assert_eq!(
        frame,
        json!({"type": "streamSubscribe", "streamId": "s", "kind": "pixel-map"})
    );
}

#[test]
fn waiting_until_the_first_sync_then_live() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    let mut client = client();

    assert_eq!(client.status(), StreamStatus::Waiting);
    assert_eq!(client.submit(id(1), paint(0, 0, 1)), Err(SubmitError::NotLive));

    client.handle_message(&server.sync(None, 0));

    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(client.version(), 1);
    assert_eq!(at(&client.view(), 1, 1), 5);
}

#[test]
fn a_sync_with_a_snapshot_and_a_tail_rebuilds_the_state() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    server.append(2, OTHER, paint(2, 2, 6));
    let snapshot = server.snapshot();
    server.append(3, OTHER, paint(3, 3, 7));
    let mut client = client();

    client.handle_message(&server.sync(Some(snapshot), 2));

    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(client.version(), 3);
    assert_eq!(client.view(), *server.replica.state());
}

#[test]
fn a_snapshot_that_does_not_match_its_hash_leaves_the_stream_unsynced() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    let mut snapshot = server.snapshot();
    snapshot["stateHash"] = json!("0000000000000001");
    let mut client = client();

    let out = client.handle_message(&server.sync(Some(snapshot), 1));

    assert_eq!(client.status(), StreamStatus::Unsynced);
    assert!(out.is_empty());
}

#[test]
fn a_second_sync_replaces_the_state_and_moves_the_revision() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    let mut client = live_client(&server);
    let before = client.revision();
    server.append(2, OTHER, paint(2, 2, 6));

    client.handle_message(&server.sync(None, 0));

    assert_eq!(client.version(), 2);
    assert_ne!(client.revision(), before);
}

#[test]
fn a_local_command_is_shown_at_once_and_stays_pending() {
    let server = Server::new();
    let mut client = live_client(&server);

    let frame = client.submit(id(1), paint(4, 5, 9)).unwrap();

    let sent: Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(sent["type"], "streamCommand");
    assert_eq!(sent["streamId"], "s");
    assert_eq!(sent["commandId"], json!(id(1)));
    assert_eq!(sent["command"], json!({"type": "paint", "x": 4, "y": 5, "color": 9}));
    assert_eq!(at(&client.view(), 4, 5), 9);
    assert_eq!(client.pending_count(), 1);
    assert_eq!(client.version(), 0);
    assert_eq!(at(client.confirmed(), 4, 5), 0);
}

#[test]
fn a_confirmed_command_clears_its_pending() {
    let mut server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(4, 5, 9)).unwrap();
    server.append(1, ME, paint(4, 5, 9));

    client.handle_message(&server.commands(0));

    assert_eq!(client.pending_count(), 0);
    assert_eq!(client.version(), 1);
    assert_eq!(at(client.confirmed(), 4, 5), 9);
    assert_eq!(client.view(), *client.confirmed());
}

#[test]
fn a_foreign_command_on_the_same_cell_does_not_hide_a_pending_one() {
    let mut server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(2, 2, 9)).unwrap();
    server.append(2, OTHER, paint(2, 2, 3));

    client.handle_message(&server.commands(0));

    assert_eq!(at(client.confirmed(), 2, 2), 3);
    assert_eq!(at(&client.view(), 2, 2), 9);

    server.append(1, ME, paint(2, 2, 9));
    client.handle_message(&server.commands(1));

    assert_eq!(client.pending_count(), 0);
    assert_eq!(at(client.confirmed(), 2, 2), 9);
}

#[test]
fn a_nack_drops_the_pending_and_the_view_reverts() {
    let server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(4, 5, 9)).unwrap();
    let before = client.revision();

    client.handle_message(&nack(1, "invalid"));

    assert_eq!(client.pending_count(), 0);
    assert_eq!(at(&client.view(), 4, 5), 0);
    assert_ne!(client.revision(), before);
}

#[test]
fn a_nack_of_an_unknown_command_changes_nothing() {
    let server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(4, 5, 9)).unwrap();
    let before = client.revision();

    client.handle_message(&nack(9, "failed"));

    assert_eq!(client.pending_count(), 1);
    assert_eq!(client.revision(), before);
}

#[test]
fn a_nack_for_not_being_subscribed_subscribes_again() {
    let server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(4, 5, 9)).unwrap();

    let out = client.handle_message(&nack(1, "notSubscribed"));

    assert_eq!(out.len(), 1);
    assert_eq!(frame_type(&out[0]), "streamSubscribe");
    assert_eq!(client.status(), StreamStatus::Waiting);
}

#[test]
fn a_sync_resubmits_only_the_pending_commands_it_does_not_contain() {
    let mut server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(1, 1, 1)).unwrap();
    client.submit(id(2), paint(2, 2, 2)).unwrap();
    server.append(1, ME, paint(1, 1, 1));

    let out = client.handle_message(&server.sync(None, 0));

    assert_eq!(out.len(), 1);
    assert_eq!(frame_type(&out[0]), "streamCommand");
    assert_eq!(command_id_of(&out[0]), id(2));
    assert_eq!(client.pending_count(), 1);
    assert_eq!(at(&client.view(), 2, 2), 2);
}

#[test]
fn a_reset_waits_for_the_next_sync_and_keeps_the_view() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    let mut client = live_client(&server);
    client.submit(id(1), paint(2, 2, 6)).unwrap();

    client.handle_message(&json!({"type": "streamReset", "streamId": "s"}).to_string());

    assert_eq!(client.status(), StreamStatus::Waiting);
    assert_eq!(at(&client.view(), 1, 1), 5);
    assert_eq!(at(&client.view(), 2, 2), 6);
    assert_eq!(client.submit(id(2), paint(0, 0, 1)), Err(SubmitError::NotLive));

    client.handle_message(&server.sync(None, 0));
    assert_eq!(client.status(), StreamStatus::Live);
}

#[test]
fn unsynced_stops_submits_and_keeps_the_pending_for_a_later_sync() {
    let server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(2, 2, 6)).unwrap();

    client.handle_message(&json!({"type": "streamUnsynced", "streamId": "s"}).to_string());

    assert_eq!(client.status(), StreamStatus::Unsynced);
    assert_eq!(client.submit(id(2), paint(0, 0, 1)), Err(SubmitError::NotLive));
    assert_eq!(client.pending_count(), 1);

    let out = client.handle_message(&server.sync(None, 0));
    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(command_id_of(&out[0]), id(1));
}

#[test]
fn a_subscribe_failure_leaves_the_stream_unsynced() {
    let mut client = client();

    client.handle_message(
        &json!({"type": "streamSubscribeFailed", "streamId": "s", "reason": "kindMismatch"}).to_string(),
    );

    assert_eq!(client.status(), StreamStatus::Unsynced);
}

#[test]
fn a_matching_checkpoint_is_silent() {
    let mut server = Server::new();
    let mut client = live_client(&server);
    server.append(1, OTHER, paint(1, 1, 5));
    client.handle_message(&server.commands(0));
    let before = client.revision();

    let out = client.handle_message(&server.checkpoint());

    assert!(out.is_empty());
    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(client.revision(), before);
}

#[test]
fn a_checkpoint_behind_the_client_is_still_checked() {
    let mut server = Server::new();
    let mut client = live_client(&server);
    server.append(1, OTHER, paint(1, 1, 5));
    let checkpoint = server.checkpoint();
    server.append(2, OTHER, paint(2, 2, 6));
    client.handle_message(&server.commands(0));
    let mut bad: Value = serde_json::from_str(&checkpoint).unwrap();
    bad["stateHash"] = json!("0000000000000001");

    let out = client.handle_message(&bad.to_string());

    assert_eq!(out.len(), 1);
    assert_eq!(frame_type(&out[0]), "streamSubscribe");
    assert_eq!(client.status(), StreamStatus::Waiting);
}

#[test]
fn a_checkpoint_of_a_version_the_client_does_not_know_is_ignored() {
    let mut server = Server::new();
    let mut client = live_client(&server);
    server.append(1, OTHER, paint(1, 1, 5));

    let out = client.handle_message(&server.checkpoint());

    assert!(out.is_empty());
    assert_eq!(client.status(), StreamStatus::Live);
}

#[test]
fn a_gap_in_the_commands_subscribes_again_instead_of_applying() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    server.append(2, OTHER, paint(2, 2, 6));
    let mut client = live_client(&Server::new());

    let out = client.handle_message(&server.commands(1));

    assert_eq!(out.len(), 1);
    assert_eq!(frame_type(&out[0]), "streamSubscribe");
    assert_eq!(client.status(), StreamStatus::Waiting);
    assert_eq!(client.version(), 0);
}

#[test]
fn repeated_commands_are_skipped() {
    let mut server = Server::new();
    server.append(1, OTHER, paint(1, 1, 5));
    let mut client = live_client(&server);
    let before = client.revision();

    let out = client.handle_message(&server.commands(0));

    assert!(out.is_empty());
    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(client.revision(), before);
}

#[test]
fn garbage_and_foreign_streams_are_ignored() {
    let server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(1, 1, 1)).unwrap();
    let before = client.revision();

    for text in [
        "{not json".to_string(),
        json!({"type": "nope"}).to_string(),
        json!({"type": "streamReset", "streamId": "other"}).to_string(),
        json!({"type": "streamCommands", "streamId": "other", "commands": []}).to_string(),
    ] {
        assert!(client.handle_message(&text).is_empty());
    }

    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(client.pending_count(), 1);
    assert_eq!(client.revision(), before);
}

#[test]
fn the_pending_commands_are_listed_in_the_order_they_were_submitted() {
    let server = Server::new();
    let mut client = live_client(&server);
    client.submit(id(1), paint(1, 1, 1)).unwrap();
    client.submit(id(2), paint(2, 2, 2)).unwrap();

    let pending: Vec<_> = client.pending_commands().cloned().collect();

    assert_eq!(pending, vec![paint(1, 1, 1), paint(2, 2, 2)]);
}

#[test]
fn a_resync_waits_for_a_new_sync_and_keeps_the_view_and_the_pending() {
    let mut server = Server::new();
    server.append(9, OTHER, paint(1, 1, 5));
    let mut client = live_client(&server);
    client.submit(id(1), paint(2, 2, 6)).unwrap();

    let frame = client.resync();

    assert_eq!(frame_type(&frame), "streamSubscribe");
    assert_eq!(client.status(), StreamStatus::Waiting);
    assert_eq!(at(&client.view(), 1, 1), 5);
    assert_eq!(client.pending_count(), 1);

    let out = client.handle_message(&server.sync(None, 0));

    assert_eq!(client.status(), StreamStatus::Live);
    assert_eq!(command_id_of(&out[0]), id(1));
}
