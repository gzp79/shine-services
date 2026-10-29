use serde_json::json;
use shine_game::replica::{parse_server_message, ClientMessage, ServerMessage};
use shine_test::test;
use uuid::Uuid;

fn parse(value: serde_json::Value) -> Option<ServerMessage> {
    parse_server_message(&value.to_string())
}

#[test]
fn a_sync_with_a_snapshot_and_commands_is_parsed() {
    let (command_id, user_id) = (Uuid::from_u128(1), Uuid::from_u128(2));

    let message = parse(json!({
        "type": "streamSync", "streamId": "room-1",
        "snapshot": {"version": 8, "chainHash": "00000000000000ff", "stateHash": "0000000000000100", "data": "AQID"},
        "commands": [{"version": 9, "commandId": command_id, "userId": user_id, "command": {"type": "add"}}]
    }));

    let Some(ServerMessage::StreamSync { stream_id, snapshot, commands }) = message else {
        panic!("not a sync: {message:?}");
    };
    assert_eq!(stream_id, "room-1");
    let snapshot = snapshot.unwrap();
    assert_eq!(
        (snapshot.version, snapshot.chain_hash.as_str(), snapshot.data.as_str()),
        (8, "00000000000000ff", "AQID")
    );
    assert_eq!(snapshot.state_hash, "0000000000000100");
    assert_eq!(
        (commands[0].version, commands[0].command_id, commands[0].user_id),
        (9, command_id, user_id)
    );
    assert_eq!(commands[0].command, json!({"type": "add"}));
}

#[test]
fn a_sync_without_a_snapshot_is_parsed() {
    let message = parse(json!({"type": "streamSync", "streamId": "s", "snapshot": null, "commands": []}));

    assert!(matches!(
        message,
        Some(ServerMessage::StreamSync { snapshot: None, .. })
    ));
}

#[test]
fn the_other_server_messages_are_parsed() {
    let command_id = Uuid::from_u128(7);

    let commands = parse(json!({"type": "streamCommands", "streamId": "s", "commands": []}));
    let checkpoint =
        parse(json!({"type": "streamCheckpoint", "streamId": "s", "version": 8, "stateHash": "00000000000000ff"}));
    let reset = parse(json!({"type": "streamReset", "streamId": "s"}));
    let unsynced = parse(json!({"type": "streamUnsynced", "streamId": "s"}));
    let nack =
        parse(json!({"type": "streamNack", "streamId": "s", "commandId": command_id, "reason": "notSubscribed"}));
    let failed = parse(json!({"type": "streamSubscribeFailed", "streamId": "s", "reason": "kindMismatch"}));

    assert!(matches!(commands, Some(ServerMessage::StreamCommands { .. })));
    assert!(matches!(
        checkpoint,
        Some(ServerMessage::StreamCheckpoint { version: 8, .. })
    ));
    assert!(matches!(reset, Some(ServerMessage::StreamReset { .. })));
    assert!(matches!(unsynced, Some(ServerMessage::StreamUnsynced { .. })));
    assert!(matches!(nack, Some(ServerMessage::StreamNack { command_id: id, .. }) if id == command_id));
    assert!(matches!(failed, Some(ServerMessage::StreamSubscribeFailed { .. })));
}

#[test]
fn unknown_or_malformed_frames_are_none() {
    assert!(parse(json!({"type": "nope"})).is_none());
    assert!(parse(json!({"type": "chat", "messages": []})).is_none());
    assert!(parse(json!({"type": "streamReset"})).is_none());
    assert!(parse_server_message("{not json").is_none());
    assert!(parse_server_message("").is_none());
}

#[test]
fn client_messages_use_the_camel_case_wire_keys() {
    let command_id = Uuid::from_u128(7);

    let subscribe = serde_json::to_value(ClientMessage::StreamSubscribe {
        stream_id: "pixel-map:s".into(),
    })
    .unwrap();
    let command = serde_json::to_value(ClientMessage::StreamCommand {
        stream_id: "s".into(),
        command_id,
        command: json!({"type": "paint"}),
    })
    .unwrap();

    assert_eq!(subscribe, json!({"type": "streamSubscribe", "streamId": "pixel-map:s"}));
    assert_eq!(
        command,
        json!({"type": "streamCommand", "streamId": "s", "commandId": command_id, "command": {"type": "paint"}})
    );
}
