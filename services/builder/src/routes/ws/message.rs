use crate::{
    models::messages::{CommandFailure, StreamCommand, StreamMessage, SubscribeFailure},
    routes::ws::socket_budget::BudgetKind,
    services::MAX_COMMAND_BYTES,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    #[validate(length(max = 255))]
    pub id: String,
    pub user_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageRequest {
    StreamSubscribe {
        stream_id: String,
    },
    StreamUnsubscribe {
        stream_id: String,
    },
    StreamCommand {
        stream_id: String,
        command_id: Uuid,
        command: String,
    },
    Ping(PingRequest),
}

impl WSMessageRequest {
    /// What the request costs from the budget of its socket.
    pub fn budget_kind(&self) -> BudgetKind {
        match self {
            Self::StreamSubscribe { .. } => BudgetKind::Subscribe,
            Self::StreamUnsubscribe { .. } | Self::StreamCommand { .. } | Self::Ping(_) => BudgetKind::Command,
        }
    }

    /// The label of the request in the metrics.
    pub fn metric_kind(&self) -> &'static str {
        match self {
            Self::StreamSubscribe { .. } => "stream_subscribe",
            Self::StreamUnsubscribe { .. } => "stream_unsubscribe",
            Self::StreamCommand { .. } => "stream_command",
            Self::Ping(_) => "ping",
        }
    }
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
    pub command: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[serde(tag = "type")]
pub enum WSMessageResponse {
    Connected {
        user_id: Uuid,
    },
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
    Ping {
        id: String,
    },
}

fn hex(hash: u64) -> String {
    format!("{hash:016x}")
}

fn wire_command(command: &StreamCommand) -> WireStreamCommand {
    WireStreamCommand {
        version: command.version,
        command_id: command.command_id,
        user_id: command.user_id,
        command: STANDARD.encode(&command.command),
    }
}

/// Decodes a command of the client; `None` if it is not valid base64 or is over `MAX_COMMAND_BYTES`.
pub fn decode_wire_command(command: &str) -> Option<Vec<u8>> {
    // Refused by the length of the text before anything is decoded.
    if command.len() > MAX_COMMAND_BYTES.div_ceil(3) * 4 {
        return None;
    }
    STANDARD
        .decode(command)
        .ok()
        .filter(|command| command.len() <= MAX_COMMAND_BYTES)
}

fn nack_reason(reason: CommandFailure) -> &'static str {
    match reason {
        CommandFailure::NotSubscribed => "notSubscribed",
        CommandFailure::Invalid => "invalid",
        CommandFailure::Failed => "failed",
        CommandFailure::Busy => "busy",
        CommandFailure::RateLimited => "rateLimited",
    }
}

fn subscribe_failure(reason: SubscribeFailure) -> &'static str {
    match reason {
        SubscribeFailure::Failed => "failed",
        SubscribeFailure::Unauthorized => "unauthorized",
        SubscribeFailure::RateLimited => "rateLimited",
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
        StreamMessage::Unsynced { .. } => WSMessageResponse::StreamUnsynced { stream_id },
        StreamMessage::CommandFailed { command_id, reason, .. } => WSMessageResponse::StreamNack {
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
