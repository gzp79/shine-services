//! The JSON frames of the stream protocol, as the web client sees them (`docs/services/builder/hub.md`).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSnapshot {
    pub version: i64,
    /// Hex text: a 64 bit number is not safe in JSON.
    pub chain_hash: String,
    pub state_hash: String,
    /// The state, base64 of its snapshot encoding.
    pub data: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub command: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    StreamSync {
        stream_id: String,
        snapshot: Option<WireSnapshot>,
        commands: Vec<WireCommand>,
    },
    StreamCommands {
        stream_id: String,
        commands: Vec<WireCommand>,
    },
    StreamCheckpoint {
        stream_id: String,
        version: i64,
        state_hash: String,
    },
    StreamReset {
        stream_id: String,
    },
    StreamUnsynced {
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

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientMessage {
    StreamSubscribe {
        stream_id: String,
    },
    StreamUnsubscribe {
        stream_id: String,
    },
    StreamCommand {
        stream_id: String,
        command_id: Uuid,
        command: serde_json::Value,
    },
}

/// Parses a frame of the server. Frames of other protocols and malformed ones are `None`: a client
/// ignores what it does not understand.
pub fn parse_server_message(text: &str) -> Option<ServerMessage> {
    serde_json::from_str(text).ok()
}
