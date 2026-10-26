use crate::models::messages::{NackReason, StreamCommand, StreamMessage, SubscribeFailure};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageRequest {
    StreamSubscribe {
        stream_id: String,
        kind: String,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireStreamSnapshot {
    pub version: i64,
    pub chain_hash: String,
    pub state_hash: String,
    pub data: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireStreamCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub command: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageResponse {
    StreamSync {
        stream_id: String,
        snapshot: Option<WireStreamSnapshot>,
        commands: Vec<WireStreamCommand>,
    },
    StreamCommands {
        stream_id: String,
        commands: Vec<WireStreamCommand>,
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
        reason: &'static str,
    },
    StreamSubscribeFailed {
        stream_id: String,
        reason: &'static str,
    },
}

fn hex(hash: u64) -> String {
    format!("{hash:016x}")
}

fn wire_command(command: &StreamCommand) -> WireStreamCommand {
    // Commands are stored as canonical JSON text; a client gets the JSON itself.
    let parsed = serde_json::from_str(&command.command).unwrap_or_else(|err| {
        tracing::error!("Stored command {} is not JSON: {err:#?}", command.version);
        serde_json::Value::Null
    });
    WireStreamCommand {
        version: command.version,
        command_id: command.command_id,
        user_id: command.user_id,
        command: parsed,
    }
}

fn nack_reason(reason: NackReason) -> &'static str {
    match reason {
        NackReason::NotSubscribed => "notSubscribed",
        NackReason::Invalid => "invalid",
        NackReason::Failed => "failed",
    }
}

fn subscribe_failure(reason: SubscribeFailure) -> &'static str {
    match reason {
        SubscribeFailure::UnknownKind => "unknownKind",
        SubscribeFailure::KindMismatch => "kindMismatch",
        SubscribeFailure::Failed => "failed",
    }
}

/// The frame of a stream message.
pub fn stream_message_to_wire(message: &StreamMessage) -> WSMessageResponse {
    let stream_id = message.stream_id().to_string();
    match message {
        StreamMessage::Sync { snapshot, commands, .. } => WSMessageResponse::StreamSync {
            stream_id,
            snapshot: snapshot.as_ref().map(|snapshot| WireStreamSnapshot {
                version: snapshot.version,
                chain_hash: hex(snapshot.chain_hash),
                state_hash: hex(snapshot.state_hash),
                data: STANDARD.encode(&snapshot.data),
            }),
            commands: commands.iter().map(wire_command).collect(),
        },
        StreamMessage::Commands { commands, .. } => WSMessageResponse::StreamCommands {
            stream_id,
            commands: commands.iter().map(wire_command).collect(),
        },
        StreamMessage::Checkpoint { version, state_hash, .. } => WSMessageResponse::StreamCheckpoint {
            stream_id,
            version: *version,
            state_hash: hex(*state_hash),
        },
        StreamMessage::Reset { .. } => WSMessageResponse::StreamReset { stream_id },
        StreamMessage::Unsynced { .. } => WSMessageResponse::StreamUnsynced { stream_id },
        StreamMessage::Nack { command_id, reason, .. } => WSMessageResponse::StreamNack {
            stream_id,
            command_id: *command_id,
            reason: nack_reason(*reason),
        },
        StreamMessage::SubscribeFailed { reason, .. } => WSMessageResponse::StreamSubscribeFailed {
            stream_id,
            reason: subscribe_failure(*reason),
        },
    }
}

#[cfg(test)]
#[path = "message_test.rs"]
mod message_test;
