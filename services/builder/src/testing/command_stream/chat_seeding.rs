use crate::{
    models::CommandStreamError,
    repositories::command_inbox::InboxId,
    repositories::command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand, StoredSnapshot, StreamHead},
    testing::command_stream::{TestLog, TestStreamId},
};
use shine_game::replica::{
    decode_snapshot, encode_command, encode_snapshot, stable_hash_of, ChatCommand, ChatState, CommandEnvelope, Replica,
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
            inbox_id: InboxId::new(1, version as u64),
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

    pub async fn persist(
        &self,
        db: &PgCommandLogDb,
        stream_id: &StreamId,
        first: i64,
        last: i64,
    ) -> Result<StreamHead, CommandStreamError> {
        let (batch, baseline) = self.command_range(first, last);
        db.append(stream_id, batch, baseline).await
    }

    /// Replays stored commands the way a reader of the log does.
    pub fn replay(snapshot: Option<&StoredSnapshot>, commands: &[StoredCommand]) -> Replica<ChatState> {
        let mut replica = match snapshot {
            Some(snapshot) => Replica::from_snapshot(
                snapshot.version,
                decode_snapshot::<ChatState>(&snapshot.data).unwrap(),
                snapshot.chain_hash,
            ),
            None => Replica::new(),
        };

        for command in commands {
            replica
                .apply_next(&command.to_envelope::<ChatState>().unwrap())
                .unwrap();
        }
        replica
    }
}

impl TestLog {
    /// Seed a new stream holding the first `count` commands of the `seed` room.
    pub async fn seeded_chat_log(&self, count: i64) -> (StreamId, ChatLog) {
        let room = ChatLog::new("seed", count);
        let id = TestStreamId::new_chat();
        self.db.ensure_stream(&id).await.unwrap();
        room.persist(&self.db, &id, 1, room.commands.len() as i64)
            .await
            .unwrap();
        (id, room)
    }

    /// Seed a stream of `count` commands with the snapshots of the given versions stored.
    pub async fn seed_chat_log_and_snapshots(&self, count: i64, snapshots: &[i64]) -> (StreamId, ChatLog) {
        let (id, room) = self.seeded_chat_log(count).await;
        for &version in snapshots {
            self.db
                .try_store_snapshot(&id, &room.snapshot_at(version))
                .await
                .unwrap();
        }
        (id, room)
    }
}
