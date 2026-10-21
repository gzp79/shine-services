use crate::models::messages::{StreamCommand, StreamMessage};
use shine_game::replay::protocol::{encode_snapshot_data, hash_to_hex, ServerFrame, WireCommand, WireSnapshot};

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
