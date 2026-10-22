use serde_json::json;
use shine_game::replay::protocol::{
    decode_snapshot_data, encode_snapshot_data, hash_from_hex, hash_to_hex, ClientFrame, ServerFrame, WireCommand,
    WireResume, WireSnapshot,
};
use shine_test::test;
use uuid::Uuid;

#[test]
fn a_subscribe_frame_is_parsed_with_and_without_resume() {
    let plain: ClientFrame = serde_json::from_value(json!({
        "type": "streamSubscribe", "streamId": "chat-global", "kind": "chat"
    }))
    .unwrap();
    assert_eq!(
        plain,
        ClientFrame::StreamSubscribe {
            stream_id: "chat-global".into(),
            kind: "chat".into(),
            resume: None
        }
    );

    let resuming: ClientFrame = serde_json::from_value(json!({
        "type": "streamSubscribe", "streamId": "chat-global", "kind": "chat",
        "resume": { "generation": 3, "version": 120 }
    }))
    .unwrap();
    assert_eq!(
        resuming,
        ClientFrame::StreamSubscribe {
            stream_id: "chat-global".into(),
            kind: "chat".into(),
            resume: Some(WireResume { generation: 3, version: 120 })
        }
    );
}

#[test]
fn the_other_client_frames_are_parsed() {
    let command_id = Uuid::from_u128(3);
    let unsubscribe: ClientFrame =
        serde_json::from_value(json!({ "type": "streamUnsubscribe", "streamId": "s" })).unwrap();
    assert_eq!(unsubscribe, ClientFrame::StreamUnsubscribe { stream_id: "s".into() });

    let command: ClientFrame = serde_json::from_value(json!({
        "type": "streamCommand", "streamId": "s", "commandId": command_id, "command": "{\"type\":\"add\"}"
    }))
    .unwrap();
    assert_eq!(
        command,
        ClientFrame::StreamCommand {
            stream_id: "s".into(),
            command_id,
            command: "{\"type\":\"add\"}".into()
        }
    );
}

#[test]
fn unknown_or_malformed_client_frames_are_refused() {
    for frame in [
        json!({ "type": "chat", "text": "hi" }),
        json!({ "type": "streamSubscribe", "kind": "chat" }),
        json!({ "type": "streamCommand", "streamId": "s", "commandId": "nope", "command": "{}" }),
        json!({ "streamId": "s" }),
    ] {
        assert!(serde_json::from_value::<ClientFrame>(frame.clone()).is_err(), "{frame}");
    }
}

#[test]
fn a_sync_frame_has_the_documented_shape() {
    let (command_id, user_id) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let frame = ServerFrame::StreamSync {
        stream_id: "s".into(),
        generation: 2,
        snapshot_every: 50,
        snapshot: Some(WireSnapshot {
            version: 50,
            hash: hash_to_hex(0xAB),
            data: encode_snapshot_data(&[1, 2, 3]),
        }),
        commands: vec![WireCommand {
            version: 51,
            command_id,
            user_id,
            command: "{}".into(),
        }],
    };
    assert_eq!(
        serde_json::to_value(&frame).unwrap(),
        json!({
            "type": "streamSync", "streamId": "s", "generation": 2, "snapshotEvery": 50,
            "snapshot": { "version": 50, "hash": "00000000000000ab", "data": "AQID" },
            "commands": [{ "version": 51, "commandId": command_id, "userId": user_id, "command": "{}" }]
        })
    );
    let empty = ServerFrame::StreamSync {
        stream_id: "s".into(),
        generation: 1,
        snapshot_every: 5,
        snapshot: None,
        commands: vec![],
    };
    assert_eq!(serde_json::to_value(&empty).unwrap()["snapshot"], json!(null));
}

#[test]
fn server_frames_round_trip() {
    let frames = [
        ServerFrame::StreamCommands {
            stream_id: "s".into(),
            generation: 1,
            commands: vec![],
        },
        ServerFrame::StreamCheckpoint {
            stream_id: "s".into(),
            version: 10,
            hash: hash_to_hex(u64::MAX),
        },
        ServerFrame::StreamReset { stream_id: "s".into() },
        ServerFrame::StreamNack {
            stream_id: "s".into(),
            command_id: Uuid::from_u128(3),
            reason: "busy".into(),
        },
        ServerFrame::StreamSubscribeFailed {
            stream_id: "s".into(),
            reason: "unknownKind".into(),
        },
    ];
    for frame in frames {
        let text = serde_json::to_string(&frame).unwrap();
        assert_eq!(serde_json::from_str::<ServerFrame>(&text).unwrap(), frame);
    }
}

#[test]
fn hashes_are_16_lower_case_hex_digits() {
    assert_eq!(hash_to_hex(0), "0000000000000000");
    assert_eq!(hash_to_hex(u64::MAX), "ffffffffffffffff");
    for hash in [0, 1, 0xDEAD_BEEF, u64::MAX] {
        assert_eq!(hash_from_hex(&hash_to_hex(hash)), Some(hash));
    }
    for bad in ["", "ff", "FFFFFFFFFFFFFFFF", "000000000000000g", "00000000000000000"] {
        assert_eq!(hash_from_hex(bad), None, "{bad}");
    }
}

#[test]
fn snapshot_data_round_trips_through_base64() {
    let bytes: Vec<u8> = (0..=255).collect();
    assert_eq!(decode_snapshot_data(&encode_snapshot_data(&bytes)), Some(bytes));
    assert_eq!(decode_snapshot_data("not base64!"), None);
}
