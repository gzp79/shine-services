//! The JSON frames of the stream websocket and of the snapshot route, shared by the server and the client.
//!
//! Stream ids and kinds are plain strings (the server validates them). A command travels as a JSON string,
//! so the client never parses it and large integers keep their value; a hash is 16 lower-case hex digits and
//! snapshot data is base64 of the postcard encoding of the state.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where a client continues a stream from: the epoch of the history it follows and the last version it applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireResume {
    pub generation: i64,
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSnapshot {
    pub version: i64,
    /// 16 lower-case hex digits.
    pub hash: String,
    /// Base64 of the encoded state.
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    /// The canonical command JSON, as text.
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientFrame {
    StreamSubscribe {
        stream_id: String,
        kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume: Option<WireResume>,
    },
    StreamUnsubscribe {
        stream_id: String,
    },
    StreamCommand {
        stream_id: String,
        command_id: Uuid,
        command: String,
    },
}

/// A `streamSync` is also the body of the snapshot route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerFrame {
    /// The state to start from and the first commands after it; replaces the client's state of the stream.
    StreamSync {
        stream_id: String,
        generation: i64,
        /// The client hashes its state at the multiples of this to check the checkpoints.
        snapshot_every: i64,
        snapshot: Option<WireSnapshot>,
        commands: Vec<WireCommand>,
    },
    StreamCommands {
        stream_id: String,
        generation: i64,
        commands: Vec<WireCommand>,
    },
    StreamCheckpoint {
        stream_id: String,
        version: i64,
        hash: String,
    },
    /// The client's view was rolled back; a fresh `streamSync` follows.
    StreamReset {
        stream_id: String,
    },
    StreamNack {
        stream_id: String,
        command_id: Uuid,
        reason: String,
    },
    StreamSubscribeFailed {
        stream_id: String,
        reason: String,
    },
}

pub fn hash_to_hex(hash: u64) -> String {
    format!("{hash:016x}")
}

/// `None` unless it is exactly 16 lower-case hex digits.
pub fn hash_from_hex(text: &str) -> Option<u64> {
    let valid = text.len() == 16 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if valid {
        u64::from_str_radix(text, 16).ok()
    } else {
        None
    }
}

pub fn encode_snapshot_data(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

pub fn decode_snapshot_data(text: &str) -> Option<Vec<u8>> {
    BASE64.decode(text).ok()
}
