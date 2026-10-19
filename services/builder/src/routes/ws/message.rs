use crate::models::messages::{StreamCommand, StreamMessage};
use serde::{Deserialize, Serialize};
use shine_game::replay::protocol::{
    encode_snapshot_data, hash_to_hex, ClientFrame, ServerFrame, WireCommand, WireSnapshot,
};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageRequest {
    Chat { text: String },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatWireComment {
    pub id: String,
    pub from: Uuid,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageResponse {
    Chat { messages: Vec<ChatWireComment> },
}

/// A frame received from the client: the chat of the old protocol, or a stream frame.
#[derive(Debug)]
pub enum Incoming {
    Chat(String),
    Stream(ClientFrame),
}

impl Incoming {
    /// `None` for anything that is not a frame of either protocol.
    pub fn parse(text: &str) -> Option<Self> {
        if let Ok(frame) = serde_json::from_str::<ClientFrame>(text) {
            return Some(Self::Stream(frame));
        }
        match serde_json::from_str::<WSMessageRequest>(text) {
            Ok(WSMessageRequest::Chat { text }) => Some(Self::Chat(text)),
            Err(_) => None,
        }
    }
}

/// A frame sent to the client.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WSMessageOut {
    Chat(WSMessageResponse),
    Stream(ServerFrame),
}

fn wire_commands(commands: &[StreamCommand]) -> Vec<WireCommand> {
    commands
        .iter()
        .map(|command| WireCommand {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            command: command.data.to_string(),
        })
        .collect()
}

/// The wire form of a stream message; also the body of the snapshot route.
pub fn stream_frame(message: StreamMessage) -> ServerFrame {
    match message {
        StreamMessage::Sync {
            stream_id,
            generation,
            snapshot_every,
            snapshot,
            commands,
        } => ServerFrame::StreamSync {
            stream_id: stream_id.to_string(),
            generation,
            snapshot_every,
            snapshot: snapshot.map(|snapshot| WireSnapshot {
                version: snapshot.version,
                hash: hash_to_hex(snapshot.hash),
                data: encode_snapshot_data(&snapshot.data),
            }),
            commands: wire_commands(&commands),
        },
        StreamMessage::Commands {
            stream_id,
            generation,
            commands,
        } => ServerFrame::StreamCommands {
            stream_id: stream_id.to_string(),
            generation,
            commands: wire_commands(&commands),
        },
        StreamMessage::Checkpoint { stream_id, version, hash } => ServerFrame::StreamCheckpoint {
            stream_id: stream_id.to_string(),
            version,
            hash: hash_to_hex(hash),
        },
        StreamMessage::Reset { stream_id } => ServerFrame::StreamReset {
            stream_id: stream_id.to_string(),
        },
        StreamMessage::Nack { stream_id, command_id, reason } => ServerFrame::StreamNack {
            stream_id: stream_id.to_string(),
            command_id,
            reason: reason.to_string(),
        },
        StreamMessage::SubscribeFailed { stream_id, reason } => ServerFrame::StreamSubscribeFailed {
            stream_id: stream_id.to_string(),
            reason: reason.to_string(),
        },
    }
}
