use crate::{
    models::CommandStreamError,
    repositories::command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand, StoredSnapshot, StreamHead},
    testing::command_stream::{replay, stream_id, TestLog},
};
use shine_game::replica::{
    encode_command, encode_snapshot, stable_hash_of, ChatCommand, ChatState, CommandEnvelope, ReplayState, Replica,
    StreamId,
};
use uuid::Uuid;

/// Seeded chat log with commands and snapshots, used for testing.
pub struct ChatLog {
    tag: String,
    default_user_id: Uuid,
    id_seed: u128,
    pub replica: Replica<ChatState>,
    pub commands: Vec<StoredCommand>,
    pub snapshots: Vec<StoredSnapshot>,
}

impl ChatLog {
    pub fn new(tag: &str, count: i64) -> Self {
        let seed = stable_hash_of(tag);
        let mut log = Self {
            tag: tag.to_string(),
            default_user_id: Uuid::from_u128(seed as u128),
            id_seed: (seed as u128) << 64,
            replica: Replica::new(),
            commands: Vec::new(),
            snapshots: Vec::new(),
        };
        for _ in 0..count {
            log.push_next();
        }
        log
    }

    fn command_id(&self, version: i64) -> Uuid {
        Uuid::from_u128(self.id_seed | version as u128)
    }

    /// Mostly new messages; every 4th command edits the message before it and some remove the one before.
    fn push_next(&mut self) {
        let version = self.replica.version() + 1;
        let command = match version {
            v if v % 4 == 0 => ChatCommand::Replace {
                id: self.command_id(v - 1),
                text: format!("{} edited {v}", self.tag),
            },
            v if v % 8 == 6 => ChatCommand::Remove { id: self.command_id(v - 1) },
            v => ChatCommand::Add {
                text: format!("{} message {v}", self.tag),
            },
        };
        let envelope = CommandEnvelope::new(
            version,
            self.command_id(version),
            self.default_user_id,
            self.replica.chain_hash(),
            command,
        );
        self.replica.apply_next(&envelope).unwrap();

        self.commands.push(StoredCommand {
            version,
            command_id: envelope.header.command_id,
            inbox_id: Some(format!("1-{version}")),
            user_id: envelope.header.user_id,
            data: encode_command::<ChatState>(&envelope.command).unwrap(),
            command_hash: envelope.command_hash,
            chain_hash: envelope.header.chain_hash,
        });
        self.snapshots.push(StoredSnapshot {
            version,
            hash: self.replica.hash(),
            chain_hash: self.replica.chain_hash(),
            data: encode_snapshot(self.replica.state()).unwrap(),
        });
    }

    /// The commands of versions `first..=last`, with the chain hash before the first of them.
    pub fn command_range(&self, first: i64, last: i64) -> (&[StoredCommand], u64) {
        (
            &self.commands[(first - 1) as usize..last as usize],
            self.chain_hash_at(first - 1),
        )
    }

    pub fn snapshot_at(&self, version: i64) -> StoredSnapshot {
        self.snapshots[(version - 1) as usize].clone()
    }

    /// The chain hash through `version`; 0 before the first command.
    pub fn chain_hash_at(&self, version: i64) -> u64 {
        match version {
            0 => 0,
            v => self.commands[(v - 1) as usize].chain_hash,
        }
    }
}

pub async fn persist_chat_logs(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    room: &ChatLog,
    first: i64,
    last: i64,
) -> Result<StreamHead, CommandStreamError> {
    let (batch, baseline) = room.command_range(first, last);
    db.append(stream_id, batch, baseline).await
}

/// Seed new persisted stream holding the first `count` commands of the `seed` room.
pub async fn seeded_chat_log(log: &TestLog, count: i64) -> (StreamId, ChatLog) {
    let room = ChatLog::new("seed", count);
    let id = stream_id(ChatState::KIND);
    log.db.ensure_stream(&id).await.unwrap();
    persist_chat_logs(&log.db, &id, &room, 1, room.commands.len() as i64)
        .await
        .unwrap();
    (id, room)
}

/// Seed a command log stream of `count` commands with the snapshots of the given versions stored.
pub async fn seed_chat_log_and_snapshots(log: &TestLog, count: i64, snapshots: &[i64]) -> (StreamId, ChatLog) {
    let (id, room) = seeded_chat_log(log, count).await;
    for &version in snapshots {
        log.db
            .try_store_snapshot(&id, &room.snapshot_at(version))
            .await
            .unwrap();
    }
    (id, room)
}

pub fn replay_chat(snapshot: Option<&StoredSnapshot>, commands: &[StoredCommand]) -> Replica<ChatState> {
    replay::<ChatState>(snapshot, commands)
}
