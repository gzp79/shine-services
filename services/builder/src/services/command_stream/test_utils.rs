//! Shared setup of the tests with real Redis and Postgres.

use crate::{
    app_config::CommandStreamConfig,
    models::messages::{StreamCommand, StreamMessage},
    repositories::{
        command_stream::redis::{RedisCommandStreamDb, RedisStreamConfig},
        create_postgres_pool, DBConfig,
    },
    services::command_stream::{
        AllowAll, CommandStreamService, IngestOutcome, StreamAccess, StreamEgress, StreamKindRegistry, StreamStores,
        WorkerCommand, WorkerExit, WorkerHandle,
    },
    settings::CommandStreamSettings,
};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use shine_game::replay::{ChatState, CommandContext, ReplayState, StableHash, StableHasher};
use shine_infra::db::{
    self,
    command_log::{PgCommandLogDb, StoredCommand, StreamId},
};
use shine_test::db::{get_pg_cns, get_redis_cns};
use std::{
    collections::HashSet,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};
use tokio::{
    sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock},
    task::JoinHandle,
    time::timeout,
};
use uuid::Uuid;

/// Tests with a service hold it shared; a test that cuts a database off (toxiproxy) holds it exclusively,
/// so the outage cannot hit the other tests of the process.
static DB_GATE: LazyLock<Arc<RwLock<()>>> = LazyLock::new(Arc::default);

/// Waits for the running tests with a service.
async fn db_outage_gate() -> OwnedRwLockWriteGuard<()> {
    DB_GATE.clone().write_owned().await
}

/// A second kind, to tell kinds apart.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counter(pub i64);

impl StableHash for Counter {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.0.stable_hash(hasher);
    }
}

impl ReplayState for Counter {
    type Command = i64;

    const KIND: &'static str = "counter";

    fn apply(&mut self, _ctx: &CommandContext, command: &i64) {
        self.0 += command;
    }
}

/// May read, may not write.
pub struct ReadOnly;

impl StreamAccess for ReadOnly {
    fn can_read(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        true
    }

    fn can_write(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        false
    }
}

pub struct Nobody;

impl StreamAccess for Nobody {
    fn can_read(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        false
    }

    fn can_write(&self, _user_id: Uuid, _stream_id: &StreamId) -> bool {
        false
    }
}

pub struct Test {
    pub service: CommandStreamService,
    pub redis: RedisCommandStreamDb,
    pub log: PgCommandLogDb,
    _gate: Option<OwnedRwLockReadGuard<()>>,
}

impl Test {
    /// Another service instance on the same stores, as a second process of the deployment.
    pub fn instance(&self) -> Test {
        self.with_access(Arc::new(AllowAll))
    }

    /// Another service on the same stores with the given access.
    pub fn with_access(&self, access: Arc<dyn StreamAccess>) -> Test {
        Test {
            service: CommandStreamService::new(self.service.stores().clone(), self.service.kinds().clone(), access),
            redis: self.redis.clone(),
            log: self.log.clone(),
            _gate: None,
        }
    }
}

/// A service on its own Redis key prefix; snapshots every 5 versions, 2 kept, commands up to 200 bytes.
pub async fn setup(access: Arc<dyn StreamAccess>) -> Option<Test> {
    setup_with(access, |_| {}).await
}

/// Like [`setup`], with the configuration adjusted by `tweak`.
pub async fn setup_with(access: Arc<dyn StreamAccess>, tweak: impl FnOnce(&mut CommandStreamConfig)) -> Option<Test> {
    let (redis_cns, sql_cns) = (get_redis_cns()?, get_pg_cns()?);
    let gate = DB_GATE.clone().read_owned().await;
    build(redis_cns, sql_cns, access, tweak, Some(gate)).await
}

/// A service on the given databases, for a test that cuts them off: it holds the outage gate, which the
/// returned guard keeps.
pub async fn setup_for_outage(
    redis_cns: String,
    sql_cns: String,
    access: Arc<dyn StreamAccess>,
    tweak: impl FnOnce(&mut CommandStreamConfig),
) -> (Option<Test>, OwnedRwLockWriteGuard<()>) {
    let gate = db_outage_gate().await;
    (build(redis_cns, sql_cns, access, tweak, None).await, gate)
}

async fn build(
    redis_cns: String,
    sql_cns: String,
    access: Arc<dyn StreamAccess>,
    tweak: impl FnOnce(&mut CommandStreamConfig),
    gate: Option<OwnedRwLockReadGuard<()>>,
) -> Option<Test> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut config = CommandStreamConfig {
        snapshot_every: 5,
        keep_snapshots: 2,
        orphan_idle_seconds: 0,
        max_command_bytes: 200,
        ..Default::default()
    };
    tweak(&mut config);
    let settings = CommandStreamSettings::from_config(&config).unwrap();

    let pg_pool = create_postgres_pool(&DBConfig {
        redis_cns: redis_cns.clone(),
        sql_cns: Some(sql_cns),
    })
    .await
    .unwrap();
    let log = PgCommandLogDb::new(&pg_pool).await.unwrap();

    let redis_pool = db::redis::create_redis_pool(&redis_cns).await.unwrap();
    let redis = RedisCommandStreamDb::new(
        &redis_pool,
        RedisStreamConfig {
            prefix: format!("cs-test-{}", Uuid::new_v4()),
            max_unpersisted: settings.redis_max_unpersisted,
            ttl_seconds: settings.redis_ttl_seconds,
        },
    );

    let kinds = StreamKindRegistry::new().with::<ChatState>().with::<Counter>();
    let service = CommandStreamService::new(
        StreamStores {
            redis: redis.clone(),
            log: log.clone(),
            settings,
        },
        kinds,
        access,
    );
    Some(Test {
        service,
        redis,
        log,
        _gate: gate,
    })
}

pub fn unique_id() -> StreamId {
    StreamId::parse(&format!("test-{}", Uuid::new_v4())).unwrap()
}

pub fn add(text: &str) -> String {
    format!(r#"{{"type":"add","text":"{text}"}}"#)
}

pub fn stored(version: i64, generation: i64) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::new_v4(),
        user_id: Uuid::nil(),
        generation,
        data: add(&format!("m{version}")),
    }
}

/// A stream in Postgres only: `count` commands in generation 1, snapshots at the given versions.
pub async fn postgres_stream(test: &Test, count: i64, snapshots: &[i64]) -> StreamId {
    let stream_id = unique_id();
    test.log.ensure_stream(&stream_id, "chat").await.unwrap();
    test.log.begin_generation(&stream_id).await.unwrap();
    let commands: Vec<_> = (1..=count).map(|v| stored(v, 1)).collect();
    test.log.append(&stream_id, &commands).await.unwrap();
    for version in snapshots {
        test.log
            .try_store_snapshot(&stream_id, *version, 0, b"s")
            .await
            .unwrap();
    }
    stream_id
}

pub async fn redis_versions(test: &Test, stream_id: &StreamId) -> Vec<i64> {
    let (_, entries) = test.redis.read_after(stream_id, 0, 1000).await.unwrap();
    entries.iter().map(|e| e.version).collect()
}

/// Short cadences: a worker step every 20 ms, commands persisted right away.
pub fn fast_cadence(config: &mut CommandStreamConfig) {
    config.sync_interval_ms = 20;
    config.persist_delay_ms = 1;
}

/// Records what the workers send; refuses everything to the connections in `refused`.
#[derive(Default)]
pub struct Recorder {
    refused: HashSet<Uuid>,
    sent: Mutex<Vec<(Uuid, StreamMessage)>>,
}

impl Recorder {
    pub fn refusing(refused: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            refused: refused.into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn sent_to(&self, connection_id: Uuid) -> Vec<StreamMessage> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| *id == connection_id)
            .map(|(_, message)| message.clone())
            .collect()
    }

    /// The commands delivered to a connection, in order, `Sync` and `Commands` alike.
    pub fn commands(&self, connection_id: Uuid) -> Vec<StreamCommand> {
        self.sent_to(connection_id)
            .iter()
            .flat_map(|message| match message {
                StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => commands.to_vec(),
                _ => vec![],
            })
            .collect()
    }

    pub fn versions(&self, connection_id: Uuid) -> Vec<i64> {
        self.commands(connection_id).iter().map(|c| c.version).collect()
    }

    /// The `(version, hash)` of the checkpoints sent to a connection.
    pub fn checkpoints(&self, connection_id: Uuid) -> Vec<(i64, u64)> {
        self.sent_to(connection_id)
            .iter()
            .filter_map(|message| match message {
                StreamMessage::Checkpoint { version, hash, .. } => Some((*version, *hash)),
                _ => None,
            })
            .collect()
    }

    pub fn resets(&self, connection_id: Uuid) -> usize {
        self.sent_to(connection_id)
            .iter()
            .filter(|message| matches!(message, StreamMessage::Reset { .. }))
            .count()
    }
}

impl StreamEgress for Recorder {
    fn send(&self, connection_id: Uuid, message: StreamMessage) -> BoxFuture<'_, bool> {
        Box::pin(async move {
            self.sent.lock().unwrap().push((connection_id, message));
            !self.refused.contains(&connection_id)
        })
    }
}

/// Waits until `condition` holds.
pub async fn eventually<F, Fut>(what: &str, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let found = timeout(Duration::from_secs(15), async {
        while !condition().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "timed out waiting for {what}");
}

/// A running worker of a stream, with one subscriber.
pub struct Running {
    pub handle: WorkerHandle,
    pub task: JoinHandle<WorkerExit>,
    pub egress: Arc<Recorder>,
    pub connection_id: Uuid,
}

impl Running {
    pub async fn stop(self) {
        self.handle.tx.send(WorkerCommand::Stop).unwrap();
        self.task.await.unwrap();
    }
}

pub fn start_worker(test: &Test, stream_id: &StreamId) -> Running {
    let kind = test.service.kinds().get("chat").unwrap();
    let egress = Arc::new(Recorder::default());
    let (handle, future) = kind.create_worker(egress.clone(), test.service.clone(), stream_id.clone());
    let task = tokio::spawn(future);
    let connection_id = Uuid::new_v4();
    handle
        .tx
        .send(WorkerCommand::AddSubscriber(connection_id, None))
        .unwrap();
    Running {
        handle,
        task,
        egress,
        connection_id,
    }
}

/// Writes a chat message as a new command.
pub async fn ingest(test: &Test, stream_id: &StreamId, text: &str) -> IngestOutcome {
    let kind = test.service.kinds().get("chat").unwrap();
    test.service
        .ingest(kind.as_ref(), stream_id, Uuid::new_v4(), Uuid::nil(), &add(text))
        .await
        .unwrap()
}
