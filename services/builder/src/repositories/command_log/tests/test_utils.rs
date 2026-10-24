use crate::{
    models::CommandLogError,
    repositories::{
        command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand, StoredSnapshot, StreamHead, StreamId},
        create_postgres_pool, DBConfig,
    },
};
use shine_game::replica::{
    decode_command, decode_snapshot, encode_command, encode_snapshot, stable_hash_of, ChatCommand, ChatState,
    CommandEnvelope, CommandHeader, ReplayState, Replica,
};
use shine_infra::db::postgres::{self, PgConnectionPool};
use std::env;
use tokio::{sync::OnceCell, task::JoinHandle};
use uuid::Uuid;

pub const KIND: &str = ChatState::KIND;

static MIGRATED: OnceCell<()> = OnceCell::const_new();

pub struct TestLog {
    pub db: PgCommandLogDb,
    pool: PgConnectionPool,
}

impl TestLog {
    /// Runs raw SQL, to damage a stream in ways the repository itself never would.
    pub async fn exec(&self, sql: &str) {
        let client = self.pool.get().await.unwrap();
        client.batch_execute(sql).await.unwrap();
    }

    /// Whether the database refuses this SQL.
    pub async fn rejects(&self, sql: &str) -> bool {
        let client = self.pool.get().await.unwrap();
        client.batch_execute(sql).await.is_err()
    }
}

pub async fn join<T>(handles: Vec<JoinHandle<T>>) -> Vec<T> {
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
}

/// A command log on the test Postgres, or `None` (the test skips) when none is configured. Tests
/// share the database and stay apart by using a stream of their own.
pub async fn create_test_log() -> Option<TestLog> {
    let Ok(cns) = env::var("SHINE_TEST_PG_CNS") else {
        tracing::warn!("Missing SHINE_TEST_PG_CNS, skipping test");
        return None;
    };
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Concurrent runners of the same migration would race on creating its objects.
    let migrate_cns = cns.clone();
    MIGRATED
        .get_or_init(|| async move {
            let config = DBConfig {
                redis_cns: String::new(),
                sql_cns: migrate_cns,
            };
            create_postgres_pool(&config)
                .await
                .expect("migrating the test database");
        })
        .await;

    let pool = postgres::create_postgres_pool(&cns).await.unwrap();
    let db = PgCommandLogDb::new(&pool).await.unwrap();
    Some(TestLog { db, pool })
}

pub fn stream_id() -> StreamId {
    StreamId::parse(&format!("t-{}", Uuid::new_v4().simple())).unwrap()
}

/// The log of a chat room, built the way the runtime builds it: every command is folded onto the
/// chain by a replica that also yields the snapshot of each version. The log is a function of `tag`
/// and the version, so a longer log of the same tag continues a shorter one, while logs of different
/// tags differ at every version.
pub struct ChatLog {
    tag: String,
    user_id: Uuid,
    id_seed: u128,
    replica: Replica<ChatState>,
    /// The stored commands of versions `1..=count`.
    pub commands: Vec<StoredCommand>,
    snapshots: Vec<StoredSnapshot>,
}

impl ChatLog {
    pub fn new(tag: &str, count: i64) -> Self {
        let seed = stable_hash_of(tag);
        let mut log = Self {
            tag: tag.to_string(),
            user_id: Uuid::from_u128(seed as u128),
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
            self.user_id,
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

    /// The replica after the last command.
    pub fn replica(&self) -> &Replica<ChatState> {
        &self.replica
    }

    /// The chain hash through `version`; 0 before the first command.
    pub fn chain_hash_at(&self, version: i64) -> u64 {
        match version {
            0 => 0,
            v => self.commands[(v - 1) as usize].chain_hash,
        }
    }

    /// The snapshot of the state after the command of `version`.
    pub fn snapshot_at(&self, version: i64) -> StoredSnapshot {
        self.snapshots[(version - 1) as usize].clone()
    }

    /// The commands of versions `first..=last`, with the chain hash before the first of them: what `append` takes.
    pub fn range(&self, first: i64, last: i64) -> (&[StoredCommand], u64) {
        (
            &self.commands[(first - 1) as usize..last as usize],
            self.chain_hash_at(first - 1),
        )
    }
}

pub async fn append_range(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    room: &ChatLog,
    first: i64,
    last: i64,
) -> Result<StreamHead, CommandLogError> {
    let (batch, baseline) = room.range(first, last);
    db.append(stream_id, batch, baseline).await
}

/// A new stream holding all the commands of `room`.
pub async fn store_room(log: &TestLog, room: &ChatLog) -> StreamId {
    let id = stream_id();
    log.db.ensure_stream(&id, KIND).await.unwrap();
    append_range(&log.db, &id, room, 1, room.commands.len() as i64)
        .await
        .unwrap();
    id
}

/// A new stream holding the first `count` commands of the `seed` room.
pub async fn seeded_stream(log: &TestLog, count: i64) -> (StreamId, ChatLog) {
    let room = ChatLog::new("seed", count);
    let id = store_room(log, &room).await;
    (id, room)
}

/// A `seed` stream of `count` commands with the snapshots of the given versions stored.
pub async fn stream_with_snapshots(log: &TestLog, count: i64, snapshots: &[i64]) -> (StreamId, ChatLog) {
    let (id, room) = seeded_stream(log, count).await;
    for &version in snapshots {
        log.db
            .try_store_snapshot(&id, &room.snapshot_at(version))
            .await
            .unwrap();
    }
    (id, room)
}

pub async fn all_commands(db: &PgCommandLogDb, stream_id: &StreamId) -> Vec<StoredCommand> {
    db.commands_after(stream_id, 0, i64::MAX).await.unwrap()
}

/// Replays stored commands the way a reader of the log does: from the snapshot when there is one,
/// from scratch otherwise. Commands up to the snapshot's version are skipped.
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
        let envelope = CommandEnvelope {
            header: CommandHeader {
                version: command.version,
                command_id: command.command_id,
                user_id: command.user_id,
                chain_hash: command.chain_hash,
            },
            command: decode_command::<ChatState>(&command.data).unwrap(),
            command_hash: command.command_hash,
        };
        replica.apply_next(&envelope).unwrap();
    }
    replica
}
