use crate::repositories::command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand, StoredSnapshot};
use shine_game::replica::{
    decode_command, decode_snapshot, CommandEnvelope, CommandHeader, ReplayState, Replica, StreamId, StreamKind,
};
use uuid::Uuid;

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
        let envelope = CommandEnvelope {
            header: CommandHeader {
                version: command.version,
                command_id: command.command_id,
                user_id: command.user_id,
                chain_hash: command.chain_hash,
            },
            command: decode_command::<S>(&command.data).unwrap(),
            command_hash: command.command_hash,
        };
        replica.apply_next(&envelope).unwrap();
    }
    replica
}
