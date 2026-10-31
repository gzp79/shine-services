use crate::{
    models::messages::StreamMessage,
    repositories::command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand, StoredSnapshot},
};
use shine_game::replica::{decode_snapshot, ReplayState, Replica, StreamId, StreamKind};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub const WAIT_NOTIFICATION: Duration = Duration::from_secs(5);

pub fn stream_id(kind: StreamKind) -> StreamId {
    StreamId::new(kind, format!("t-{}", Uuid::new_v4().simple())).unwrap()
}

pub async fn fetch_all_commands(db: &PgCommandLogDb, stream_id: &StreamId) -> Vec<StoredCommand> {
    db.commands_after(stream_id, 0, i64::MAX).await.unwrap()
}

/// Replays stored commands the way a reader of the log does.
pub fn replay<S: ReplayState>(snapshot: Option<&StoredSnapshot>, commands: &[StoredCommand]) -> Replica<S> {
    let mut replica = match snapshot {
        Some(snapshot) => Replica::from_snapshot(
            snapshot.version,
            decode_snapshot::<S>(&snapshot.data).unwrap(),
            snapshot.chain_hash,
        ),
        None => Replica::new(),
    };

    for command in commands {
        replica.apply_next(&command.to_envelope::<S>().unwrap()).unwrap();
    }
    replica
}

pub fn message_kinds(messages: &[Arc<StreamMessage>]) -> Vec<String> {
    messages
        .iter()
        .map(|m| match m.as_ref() {
            StreamMessage::Sync { commands, .. } => format!("sync:{}", commands.len()),
            StreamMessage::Commands { commands, .. } => format!("commands:{}", commands.len()),
            StreamMessage::Checkpoint { version, .. } => format!("checkpoint:{version}"),
            StreamMessage::Reset { .. } => "reset".to_string(),
            StreamMessage::Unsynced { .. } => "unsynced".to_string(),
            StreamMessage::CommandFailed { .. } => "command-failed".to_string(),
            StreamMessage::SubscribeFailed { .. } => "subscribe-failed".to_string(),
        })
        .collect()
}

/// The versions of the commands for a given stream in the order they appear in sync and command messages.
pub fn message_versions(messages: &[Arc<StreamMessage>], stream_id: &StreamId) -> Vec<i64> {
    let mut versions = Vec::new();
    for message in messages.iter().filter(|m| m.stream_id() == stream_id) {
        match message.as_ref() {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                versions.extend(commands.iter().map(|c| c.version));
            }
            _ => {}
        }
    }
    versions
}
