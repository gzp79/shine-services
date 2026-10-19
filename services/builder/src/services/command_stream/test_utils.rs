//! Shared setup of the tests with real Redis and Postgres.

use crate::{
    app_config::CommandStreamConfig,
    repositories::{
        command_stream::redis::{RedisCommandStreamDb, RedisStreamConfig},
        create_postgres_pool, DBConfig,
    },
    services::command_stream::{CommandStreamService, StreamAccess, StreamKindRegistry, StreamStores},
    settings::CommandStreamSettings,
};
use serde::{Deserialize, Serialize};
use shine_game::replay::{ChatState, CommandContext, ReplayState, StableHash, StableHasher};
use shine_infra::db::{
    self,
    command_log::{PgCommandLogDb, StoredCommand, StreamId},
};
use shine_test::db::{get_pg_cns, get_redis_cns};
use std::sync::Arc;
use uuid::Uuid;

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
}

/// A service on its own Redis key prefix; snapshots every 5 versions, 2 kept, commands up to 200 bytes.
pub async fn setup(access: Arc<dyn StreamAccess>) -> Option<Test> {
    setup_with(access, |_| {}).await
}

/// Like [`setup`], with the configuration adjusted by `tweak`.
pub async fn setup_with(access: Arc<dyn StreamAccess>, tweak: impl FnOnce(&mut CommandStreamConfig)) -> Option<Test> {
    setup_on(get_redis_cns()?, access, tweak).await
}

/// Like [`setup_with`], on the given Redis.
pub async fn setup_on(
    redis_cns: String,
    access: Arc<dyn StreamAccess>,
    tweak: impl FnOnce(&mut CommandStreamConfig),
) -> Option<Test> {
    let sql_cns = get_pg_cns()?;
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
    Some(Test { service, redis, log })
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
