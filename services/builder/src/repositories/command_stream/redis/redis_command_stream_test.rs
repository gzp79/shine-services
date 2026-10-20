use crate::repositories::command_stream::redis::{MarkOutcome, RedisCommandStreamDb, RedisIngest, RedisStreamConfig};
use shine_infra::db::{
    self,
    command_log::{StoredCommand, StreamId},
};
use shine_test::{db::get_redis_cns, test};
use std::time::Duration;
use tokio::{sync::mpsc, time::timeout};
use uuid::Uuid;

/// A repository on its own key prefix, so tests never share keys, the activity set or the wake channel.
async fn setup(max_unpersisted: i64) -> Option<RedisCommandStreamDb> {
    let cns = get_redis_cns()?;
    let pool = db::redis::create_redis_pool(&cns).await.unwrap();
    let config = RedisStreamConfig {
        prefix: format!("cs-test-{}", Uuid::new_v4()),
        max_unpersisted,
        ttl_seconds: 600,
    };
    Some(RedisCommandStreamDb::new(&pool, config))
}

fn unique_id() -> StreamId {
    StreamId::parse(&format!("test-{}", Uuid::new_v4())).unwrap()
}

/// Ingests `count` commands and returns their ids, in version order.
async fn ingest_many(redis: &RedisCommandStreamDb, stream_id: &StreamId, count: usize) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for _ in 0..count {
        let command_id = Uuid::new_v4();
        assert!(matches!(
            redis.ingest(stream_id, command_id, Uuid::nil(), "{}").await.unwrap(),
            RedisIngest::Accepted(_)
        ));
        ids.push(command_id);
    }
    ids
}

async fn versions(redis: &RedisCommandStreamDb, stream_id: &StreamId) -> Vec<i64> {
    let (_, entries) = redis.read_after(stream_id, 0, 100).await.unwrap();
    entries.iter().map(|e| e.version).collect()
}

fn stored(version: i64) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        generation: 1,
        data: r#"{"type":"add","text":"hi"}"#.into(),
    }
}

#[test]
async fn ingest_requires_hydration_and_assigns_dense_versions() {
    let Some(redis) = setup(3).await else { return };
    let stream_id = unique_id();
    let user = Uuid::new_v4();
    assert_eq!(
        redis.ingest(&stream_id, Uuid::new_v4(), user, "{}").await.unwrap(),
        RedisIngest::NotReady
    );

    assert!(redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap());
    assert_eq!(
        redis.ingest(&stream_id, Uuid::new_v4(), user, "{}").await.unwrap(),
        RedisIngest::Accepted(1)
    );
    assert_eq!(
        redis.ingest(&stream_id, Uuid::new_v4(), user, "{}").await.unwrap(),
        RedisIngest::Accepted(2)
    );

    let (meta, entries) = redis.read_after(&stream_id, 0, 10).await.unwrap();
    let meta = meta.unwrap();
    assert_eq!((meta.generation, meta.version, meta.persisted), (1, 2, 0));
    assert_eq!(entries.iter().map(|e| e.version).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(redis.read_after(&stream_id, 1, 10).await.unwrap().1.len(), 1);
}

#[test]
async fn ingest_is_busy_when_persistence_lags() {
    let Some(redis) = setup(3).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, &stream_id, 3).await;
    assert_eq!(
        redis
            .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
            .await
            .unwrap(),
        RedisIngest::Busy
    );
    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 3, ids[2], 1).await.unwrap(),
        MarkOutcome::Marked
    );
    assert_eq!(
        redis
            .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
            .await
            .unwrap(),
        RedisIngest::Accepted(4)
    );
}

#[test]
async fn mark_persisted_never_trims_unpersisted_commands() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, &stream_id, 6).await;

    // The oldest snapshot is at 100, far above what is persisted: only the persisted 1..=3 may go.
    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 3, ids[2], 100).await.unwrap(),
        MarkOutcome::Marked
    );
    assert_eq!(versions(&redis, &stream_id).await, vec![4, 5, 6]);
    assert_eq!(
        redis.ingest(&stream_id, ids[0], Uuid::nil(), "{}").await.unwrap(),
        RedisIngest::Accepted(7),
        "the dedupe entries of the trimmed commands go with them"
    );
}

#[test]
async fn mark_persisted_refuses_a_copy_that_disagrees() {
    // A copy that went back in time holds other commands at the persisted versions, or none.
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, &stream_id, 3).await;
    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 2, Uuid::new_v4(), 1).await.unwrap(),
        MarkOutcome::Refused,
        "another command at the version"
    );
    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 4, ids[2], 1).await.unwrap(),
        MarkOutcome::Refused,
        "behind the version"
    );
    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 2, ids[1], 1).await.unwrap(),
        MarkOutcome::Marked
    );
}

#[test]
async fn hydrate_copies_tail_and_newer_generation_wins() {
    let Some(redis) = setup(3).await else { return };
    let stream_id = unique_id();
    let tail = vec![stored(9), stored(10)];
    assert!(redis.hydrate(&stream_id, 2, 10, &tail).await.unwrap());
    assert!(
        !redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap(),
        "older generation is skipped"
    );
    assert!(
        !redis.hydrate(&stream_id, 2, 0, &[]).await.unwrap(),
        "same generation is skipped"
    );

    let (meta, entries) = redis.read_after(&stream_id, 0, 10).await.unwrap();
    let meta = meta.unwrap();
    assert_eq!((meta.generation, meta.version, meta.persisted), (2, 10, 10));
    assert_eq!(
        entries.iter().map(|e| e.command_id).collect::<Vec<_>>(),
        vec![tail[0].command_id, tail[1].command_id]
    );
    assert_eq!(
        redis
            .ingest(&stream_id, tail[1].command_id, Uuid::nil(), "{}")
            .await
            .unwrap(),
        RedisIngest::Duplicate(10),
        "hydrated commands keep their dedupe entries"
    );
    assert_eq!(
        redis
            .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
            .await
            .unwrap(),
        RedisIngest::Accepted(11)
    );

    assert!(
        redis.hydrate(&stream_id, 3, 10, &[]).await.unwrap(),
        "newer generation overwrites"
    );
    assert_eq!(redis.meta(&stream_id).await.unwrap().unwrap().version, 10);
}

#[test]
async fn reset_requires_matching_generation() {
    let Some(redis) = setup(3).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 4, 0, &[]).await.unwrap();
    assert!(!redis.reset(&stream_id, 3).await.unwrap());
    assert!(redis.meta(&stream_id).await.unwrap().is_some());
    assert!(redis.reset(&stream_id, 4).await.unwrap());
    assert!(redis.meta(&stream_id).await.unwrap().is_none());
}

#[test]
async fn hydration_lock_is_exclusive() {
    let Some(redis) = setup(3).await else { return };
    let stream_id = unique_id();
    let token = redis.try_lock_hydration(&stream_id).await.unwrap().unwrap();
    assert!(redis.try_lock_hydration(&stream_id).await.unwrap().is_none());
    redis.unlock_hydration(&stream_id, "not-the-token").await.unwrap();
    assert!(redis.try_lock_hydration(&stream_id).await.unwrap().is_none());
    redis.unlock_hydration(&stream_id, &token).await.unwrap();
    assert!(redis.try_lock_hydration(&stream_id).await.unwrap().is_some());
}

#[test]
async fn the_activity_set_keeps_streams_until_fully_persisted() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, &stream_id, 1).await;
    assert_eq!(
        redis.idle_streams(Duration::ZERO, 10).await.unwrap(),
        vec![stream_id.clone()]
    );

    assert!(
        !redis.retire_active(&stream_id).await.unwrap(),
        "version 1 is not persisted yet"
    );
    redis.requeue_active(&stream_id).await.unwrap();
    assert_eq!(
        redis.idle_streams(Duration::ZERO, 10).await.unwrap(),
        vec![stream_id.clone()]
    );

    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 1, ids[0], 1).await.unwrap(),
        MarkOutcome::Marked
    );
    assert!(redis.retire_active(&stream_id).await.unwrap());
    assert!(redis.idle_streams(Duration::ZERO, 10).await.unwrap().is_empty());

    redis
        .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
        .await
        .unwrap();
    assert_eq!(
        redis.idle_streams(Duration::ZERO, 10).await.unwrap(),
        vec![stream_id],
        "a new write lists it again"
    );
}

#[test]
async fn ingest_wakes_listeners_with_the_stream_id() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    redis
        .listen_to_wakes(move |woken| {
            let _ = tx.send(woken);
        })
        .await
        .unwrap();

    // The subscription is asynchronous: keep writing until the first wake arrives.
    let woken = timeout(Duration::from_secs(5), async {
        loop {
            redis
                .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
                .await
                .unwrap();
            if let Ok(Some(woken)) = timeout(Duration::from_millis(100), rx.recv()).await {
                return woken;
            }
        }
    })
    .await
    .expect("no wake within 5s");
    assert_eq!(woken, Some(stream_id));
}

#[test]
async fn hydrate_rejects_an_inconsistent_tail_without_touching_the_stream() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    ingest_many(&redis, &stream_id, 3).await;
    let before = redis.read_after(&stream_id, 0, 10).await.unwrap();

    let duplicate = vec![stored(4), stored(4)];
    let descending = vec![stored(5), stored(4)];
    let gap = vec![stored(4), stored(6)];
    let beyond_the_head = vec![stored(4), stored(5)];
    let below_one = vec![stored(0)];
    for (head, tail) in [
        (5, &duplicate),
        (5, &descending),
        (6, &gap),
        (4, &beyond_the_head),
        (0, &below_one),
    ] {
        assert!(
            redis.hydrate(&stream_id, 2, head, tail).await.is_err(),
            "head {head}, versions {:?}",
            tail.iter().map(|c| c.version).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        redis.read_after(&stream_id, 0, 10).await.unwrap(),
        before,
        "nothing was written"
    );
}

#[test]
async fn concurrent_ingests_assign_dense_unique_versions() {
    let Some(redis) = setup(1000).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();

    let tasks: Vec<_> = (0..50)
        .map(|_| {
            let (redis, stream_id) = (redis.clone(), stream_id.clone());
            tokio::spawn(async move {
                match redis
                    .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
                    .await
                    .unwrap()
                {
                    RedisIngest::Accepted(version) => version,
                    other => panic!("unexpected outcome {other:?}"),
                }
            })
        })
        .collect();
    let mut assigned = Vec::new();
    for task in tasks {
        assigned.push(task.await.unwrap());
    }
    assigned.sort();
    assert_eq!(assigned, (1..=50).collect::<Vec<_>>());
    assert_eq!(versions(&redis, &stream_id).await, (1..=50).collect::<Vec<_>>());
}

#[test]
async fn concurrent_ingests_of_one_command_are_accepted_once() {
    let Some(redis) = setup(1000).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let command_id = Uuid::new_v4();

    let tasks: Vec<_> = (0..20)
        .map(|_| {
            let (redis, stream_id) = (redis.clone(), stream_id.clone());
            tokio::spawn(async move { redis.ingest(&stream_id, command_id, Uuid::nil(), "{}").await.unwrap() })
        })
        .collect();
    let mut accepted = 0;
    for task in tasks {
        match task.await.unwrap() {
            RedisIngest::Accepted(1) => accepted += 1,
            RedisIngest::Duplicate(1) => {}
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(versions(&redis, &stream_id).await, vec![1]);
}

#[test]
async fn the_keys_of_a_stream_expire_after_the_ttl() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    let keys = redis.keys(&stream_id);
    let mut client = redis.0.pool.get().await.unwrap();

    async fn ttl(client: &mut redis::aio::MultiplexedConnection, key: &str) -> i64 {
        redis::cmd("TTL").arg(key).query_async(client).await.unwrap()
    }

    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    assert!(
        (1..=600).contains(&ttl(&mut client, &keys.meta).await),
        "hydrate sets the TTL of the meta"
    );

    ingest_many(&redis, &stream_id, 1).await;
    for key in [&keys.meta, &keys.log, &keys.ids] {
        let remaining = ttl(&mut client, key).await;
        assert!((1..=600).contains(&remaining), "{key}: {remaining}");
    }
}

#[test]
async fn mark_persisted_trims_exactly_below_keep_from() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, &stream_id, 6).await;

    assert_eq!(
        redis.mark_persisted(&stream_id, 2, 5, ids[4], 1).await.unwrap(),
        MarkOutcome::Unchanged,
        "other generation"
    );

    // Persisted up to 3, the oldest stored snapshot is at 1: nothing may go.
    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 3, ids[2], 1).await.unwrap(),
        MarkOutcome::Marked
    );
    assert_eq!(versions(&redis, &stream_id).await, vec![1, 2, 3, 4, 5, 6]);

    assert_eq!(
        redis.mark_persisted(&stream_id, 1, 6, ids[5], 3).await.unwrap(),
        MarkOutcome::Marked
    );
    assert_eq!(
        versions(&redis, &stream_id).await,
        vec![3, 4, 5, 6],
        "version 3 itself stays"
    );
    assert_eq!(
        redis.ingest(&stream_id, ids[2], Uuid::nil(), "{}").await.unwrap(),
        RedisIngest::Duplicate(3),
        "the dedupe entry of the kept version stays"
    );
    assert_eq!(
        redis.ingest(&stream_id, ids[1], Uuid::nil(), "{}").await.unwrap(),
        RedisIngest::Accepted(7),
        "the dedupe entry of the trimmed version is gone"
    );
}

#[test]
async fn idle_streams_respects_the_idle_time() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    ingest_many(&redis, &stream_id, 1).await;

    assert!(
        redis
            .idle_streams(Duration::from_secs(60), 10)
            .await
            .unwrap()
            .is_empty(),
        "a stream just written is not idle for a minute"
    );
    assert_eq!(redis.idle_streams(Duration::ZERO, 10).await.unwrap(), vec![stream_id]);
}

#[test]
async fn a_stream_that_was_reset_can_be_retired() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    ingest_many(&redis, &stream_id, 2).await;
    assert!(redis.reset(&stream_id, 1).await.unwrap());

    assert!(
        redis.retire_active(&stream_id).await.unwrap(),
        "nothing is unpersisted in a missing stream"
    );
    assert!(redis.idle_streams(Duration::ZERO, 10).await.unwrap().is_empty());
}

#[test]
async fn payloads_survive_the_round_trip_byte_for_byte() {
    let Some(redis) = setup(100).await else { return };
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    let data = "{\"text\":\"line\\nbreak \u{2713} \\u0000 \u{0} end\"}";
    let command_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    redis.ingest(&stream_id, command_id, user_id, data).await.unwrap();

    let (_, entries) = redis.read_after(&stream_id, 0, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].command_id, entries[0].user_id), (command_id, user_id));
    assert_eq!(entries[0].data, data);
}

#[test]
async fn a_zero_ttl_cannot_delete_the_keys() {
    let Some(cns) = get_redis_cns() else { return };
    let pool = db::redis::create_redis_pool(&cns).await.unwrap();
    let redis = RedisCommandStreamDb::new(
        &pool,
        RedisStreamConfig {
            prefix: format!("cs-test-{}", Uuid::new_v4()),
            max_unpersisted: 0,
            ttl_seconds: 0,
        },
    );
    let stream_id = unique_id();
    redis.hydrate(&stream_id, 1, 0, &[]).await.unwrap();
    assert_eq!(
        redis
            .ingest(&stream_id, Uuid::new_v4(), Uuid::nil(), "{}")
            .await
            .unwrap(),
        RedisIngest::Accepted(1),
        "a configured ttl of 0 is raised to 1 second, and max_unpersisted of 0 to 1"
    );
    assert!(redis.meta(&stream_id).await.unwrap().is_some());
}
