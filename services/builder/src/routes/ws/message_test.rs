//! The JSON frames of the stream messages: what a client sends and what it is sent.

use super::*;
use crate::models::messages::{CommandFailure, StreamCommand, StreamMessage, StreamSnapshot, SubscribeFailure};
use serde_json::json;
use shine_game::replica::{StreamId, StreamKind};
use shine_test::test;

fn stream_id(name: &str) -> StreamId {
    StreamId::new(StreamKind::Chat, name).unwrap()
}

fn frame(message: StreamMessage) -> serde_json::Value {
    serde_json::to_value(stream_message_to_wire(&message)).unwrap()
}

#[test]
fn a_sync_carries_the_snapshot_and_the_commands() {
    let (command_id, user_id) = (Uuid::new_v4(), Uuid::new_v4());

    let json = frame(StreamMessage::Sync {
        stream_id: stream_id("room-1"),
        snapshot: Some(StreamSnapshot {
            version: 8,
            chain_hash: 0xAB,
            state_hash: u64::MAX,
            data: vec![1, 2, 3],
        }),
        commands: vec![StreamCommand {
            version: 9,
            command_id,
            user_id,
            command: r#"{"type":"add","text":"hi"}"#.to_string(),
        }],
    });

    assert_eq!(
        json,
        json!({
            "type": "streamSync",
            "streamId": "chat:room-1",
            "snapshot": {"version": 8, "chainHash": "00000000000000ab", "stateHash": "ffffffffffffffff", "data": "AQID"},
            "commands": [{"version": 9, "commandId": command_id, "userId": user_id, "command": {"type": "add", "text": "hi"}}]
        })
    );
}

#[test]
fn a_sync_without_a_snapshot_has_a_null_one() {
    let json = frame(StreamMessage::Sync {
        stream_id: stream_id("room-1"),
        snapshot: None,
        commands: vec![],
    });

    assert_eq!(
        json,
        json!({"type": "streamSync", "streamId": "chat:room-1", "snapshot": null, "commands": []})
    );
}

#[test]
fn the_other_stream_messages_have_their_own_frames() {
    let command_id = Uuid::new_v4();

    assert_eq!(
        frame(StreamMessage::Commands {
            stream_id: stream_id("s"),
            commands: vec![StreamCommand {
                version: 3,
                command_id,
                user_id: Uuid::nil(),
                command: "{\"type\":\"paint\",\"x\":1,\"y\":2,\"color\":3}".to_string(),
            }]
        })["commands"][0]["command"]["color"],
        3
    );
    assert_eq!(
        frame(StreamMessage::Checkpoint {
            stream_id: stream_id("s"),
            version: 8,
            state_hash: 255
        }),
        json!({"type": "streamCheckpoint", "streamId": "chat:s", "version": 8, "stateHash": "00000000000000ff"})
    );
    assert_eq!(
        frame(StreamMessage::Reset { stream_id: stream_id("s") }),
        json!({"type": "streamReset", "streamId": "chat:s"})
    );
    assert_eq!(
        frame(StreamMessage::Unsynced { stream_id: stream_id("s") }),
        json!({"type": "streamUnsynced", "streamId": "chat:s"})
    );
    assert_eq!(
        frame(StreamMessage::CommandFailed {
            stream_id: stream_id("s"),
            command_id,
            reason: CommandFailure::NotSubscribed
        }),
        json!({"type": "streamNack", "streamId": "chat:s", "commandId": command_id, "reason": "notSubscribed"})
    );
    assert_eq!(
        frame(StreamMessage::SubscribeFailed {
            stream_id: stream_id("s"),
            reason: SubscribeFailure::Failed
        }),
        json!({"type": "streamSubscribeFailed", "streamId": "chat:s", "reason": "failed"})
    );
}
