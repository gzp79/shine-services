//! Several workers and instances on real Redis and Postgres: ordering, snapshot disagreement, losing or
//! rolling back Redis, and a Postgres outage.

use crate::{
    app_config::KindRetentionConfig,
    models::messages::{StreamMessage, StreamResume},
    repositories::command_stream::redis::RedisStreamEntry,
    services::command_stream::{
        test_utils::{
            add, eventually, fast_cadence, ingest, redis_versions, setup_for_outage, setup_with, start_worker,
            unique_id, Running, Test,
        },
        AllowAll, IngestOutcome, WorkerCommand,
    },
};
use shine_game::replay::{encode_snapshot, stable_hash_of, ChatState};
use shine_infra::db::command_log::{StoredCommand, StreamId};
use shine_test::{
    db::{get_pg_cns, get_redis_cns},
    test,
    toxiproxy::Toxiproxy,
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

async fn open(test: &Test) -> StreamId {
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    stream_id
}

fn command_ids(running: &Running) -> Vec<Uuid> {
    running
        .egress
        .commands(running.connection_id)
        .iter()
        .map(|c| c.command_id)
        .collect()
}

#[test]
async fn two_instances_order_and_deliver_the_same_commands() {
    let Some(first) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let second = first.instance();
    let stream_id = open(&first).await;
    let (worker_a, worker_b) = (start_worker(&first, &stream_id), start_worker(&second, &stream_id));

    let write = |test: &Test, tag: &'static str| {
        let (test, stream_id) = (test.instance(), stream_id.clone());
        async move {
            for i in 0..10 {
                let outcome = ingest(&test, &stream_id, &format!("{tag}{i}")).await;
                assert!(matches!(outcome, IngestOutcome::Accepted(_)), "{outcome:?}");
            }
        }
    };
    tokio::join!(write(&first, "a"), write(&second, "b"));

    eventually("both instances deliver every version", || async {
        let all: Vec<i64> = (1..=20).collect();
        worker_a.egress.versions(worker_a.connection_id) == all
            && worker_b.egress.versions(worker_b.connection_id) == all
    })
    .await;
    assert_eq!(command_ids(&worker_a), command_ids(&worker_b), "one order for everyone");

    eventually("everything persisted once", || async {
        first.log.head(&stream_id).await.unwrap().unwrap().version == 20
            && first.redis.meta(&stream_id).await.unwrap().unwrap().persisted == 20
    })
    .await;
    // Snapshots prune the log from the oldest kept one on: what Postgres still holds is the delivered order.
    let stored: Vec<Uuid> = first
        .log
        .commands_after(&stream_id, 0, 100)
        .await
        .unwrap()
        .iter()
        .map(|c| c.command_id)
        .collect();
    assert!(
        command_ids(&worker_a).ends_with(&stored),
        "Postgres holds the delivered order, without duplicates"
    );
    assert!(!stored.is_empty());

    worker_a.stop().await;
    worker_b.stop().await;
}

#[test]
async fn a_snapshot_that_disagrees_with_the_stored_one_is_replaced_by_it() {
    // The worker persists only when it stops, so another instance stores its snapshot first.
    let Some(test) = setup_with(Arc::new(AllowAll), |config| {
        config.sync_interval_ms = 60_000;
        config.persist_delay_ms = 60_000;
    })
    .await
    else {
        return;
    };
    let stream_id = open(&test).await;
    let running = start_worker(&test, &stream_id);
    eventually("the initial sync", || async {
        !running.egress.sent_to(running.connection_id).is_empty()
    })
    .await;
    for i in 1..=6 {
        ingest(&test, &stream_id, &format!("m{i}")).await;
    }
    running.handle.wake.notify_one();
    eventually("every command delivered", || async {
        running.egress.versions(running.connection_id) == (1..=6).collect::<Vec<_>>()
    })
    .await;

    // Another instance persists the commands and stores a different state at version 5: it is authoritative.
    test.instance().service.sweep_orphans().await;
    let authoritative = ChatState::default();
    let hash = stable_hash_of(&authoritative);
    test.log
        .try_store_snapshot(&stream_id, 5, hash, &encode_snapshot(&authoritative).unwrap())
        .await
        .unwrap();

    // The final persist of the worker finds the disagreement and takes the stored state.
    let (egress, connection_id) = (running.egress.clone(), running.connection_id);
    running.stop().await;
    assert_eq!(egress.checkpoints(connection_id), vec![(5, hash)]);
    assert_eq!(
        test.log.latest_snapshot(&stream_id, 6).await.unwrap().unwrap().hash,
        hash,
        "the stored snapshot is not overwritten"
    );
}

#[test]
async fn a_copy_of_another_generation_is_dropped_by_the_sweep() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = open(&test).await;
    ingest(&test, &stream_id, "never persisted").await;
    // Another instance rebuilt the stream: this copy is a fenced-off writer.
    test.log.begin_generation(&stream_id).await.unwrap();

    test.service.sweep_orphans().await;

    assert!(
        test.redis.meta(&stream_id).await.unwrap().is_none(),
        "the stale copy is dropped"
    );
    assert_eq!(
        test.log.head(&stream_id).await.unwrap().unwrap().version,
        0,
        "and never reaches Postgres"
    );
}

#[test]
async fn redis_loss_continues_from_the_postgres_head() {
    // Nothing is persisted on a timer, so the commands exist only in the worker and in Redis.
    let Some(test) = setup_with(Arc::new(AllowAll), |config| {
        config.sync_interval_ms = 60_000;
        config.persist_delay_ms = 60_000;
    })
    .await
    else {
        return;
    };
    let stream_id = open(&test).await;
    let running = start_worker(&test, &stream_id);
    eventually("the initial sync", || async {
        !running.egress.sent_to(running.connection_id).is_empty()
    })
    .await;
    ingest(&test, &stream_id, "a").await;
    ingest(&test, &stream_id, "b").await;
    running.handle.wake.notify_one();
    eventually("both commands delivered", || async {
        running.egress.versions(running.connection_id) == [1, 2]
    })
    .await;
    assert_eq!(test.log.head(&stream_id).await.unwrap().unwrap().version, 0);

    // Redis loses the stream.
    let before = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert!(test.redis.reset(&stream_id, before.generation).await.unwrap());
    running.handle.wake.notify_one();

    eventually(
        "the worker persisted what only it held, and Redis was rebuilt",
        || async {
            let persisted = test.log.head(&stream_id).await.unwrap().unwrap().version == 2;
            let rebuilt = test.redis.meta(&stream_id).await.unwrap();
            persisted && rebuilt.is_some_and(|meta| meta.generation > before.generation && meta.version == 2)
        },
    )
    .await;
    eventually("versions continue from the Postgres head", || async {
        matches!(ingest(&test, &stream_id, "c").await, IngestOutcome::Accepted(3))
    })
    .await;
    running.handle.wake.notify_one();
    eventually("the next command is delivered without a gap", || async {
        running.egress.versions(running.connection_id) == [1, 2, 3]
    })
    .await;
    assert_eq!(
        running.egress.resets(running.connection_id),
        0,
        "nothing was lost, nobody is reset"
    );
    running.stop().await;
}

#[test]
async fn redis_rolled_back_within_a_generation_is_repaired() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = open(&test).await;
    let running = start_worker(&test, &stream_id);
    for text in ["a", "b", "c"] {
        ingest(&test, &stream_id, text).await;
    }
    eventually("everything persisted", || async {
        test.log.head(&stream_id).await.unwrap().unwrap().version == 3
    })
    .await;
    let original = test.log.commands_after(&stream_id, 0, 10).await.unwrap();

    // Redis comes back from an old dump in the same generation, and assigned version 3 to another command.
    let meta = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert!(test.redis.reset(&stream_id, meta.generation).await.unwrap());
    let mut tail = original[..2].to_vec();
    tail.push(StoredCommand {
        version: 3,
        command_id: Uuid::new_v4(),
        user_id: Uuid::nil(),
        generation: meta.generation,
        data: add("impostor"),
    });
    assert!(test.redis.hydrate(&stream_id, meta.generation, 3, &tail).await.unwrap());

    eventually("the clients were reset and synced with the real history", || async {
        let commands = running.egress.commands(running.connection_id);
        running.egress.resets(running.connection_id) >= 1
            && commands.len() >= 6
            && commands[commands.len() - 3..]
                .iter()
                .map(|c| c.command_id)
                .eq(original.iter().map(|c| c.command_id))
    })
    .await;
    let rebuilt: Vec<RedisStreamEntry> = test.redis.read_after(&stream_id, 0, 10).await.unwrap().1;
    assert_eq!(
        rebuilt.last().map(|e| e.command_id),
        original.last().map(|c| c.command_id)
    );
    let sent = running.egress.sent_to(running.connection_id);
    let reset_at = sent
        .iter()
        .position(|m| matches!(m, StreamMessage::Reset { .. }))
        .unwrap();
    assert!(
        matches!(sent[reset_at + 1], StreamMessage::Sync { .. }),
        "a reset is followed by a full sync"
    );
    running.stop().await;
}

#[test(serial = "hub-service")]
async fn a_postgres_outage_does_not_stop_the_process_and_persistence_resumes() {
    let Some(toxiproxy) = Toxiproxy::from_env() else { return };
    let (Some(redis_cns), Some(pg_cns)) = (get_redis_cns(), get_pg_cns()) else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let proxy = toxiproxy.proxy("postgres").await;
    // Short timeouts, so the outage fails fast.
    let pg_cns = format!("{pg_cns}&connect_timeout=1&pool_timeout=2");
    let (Some(test), _outage) = setup_for_outage(redis_cns, pg_cns, Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = open(&test).await;
    let running = start_worker(&test, &stream_id);
    ingest(&test, &stream_id, "before").await;
    eventually("the first command persisted", || async {
        test.log.head(&stream_id).await.unwrap().unwrap().version == 1
    })
    .await;

    proxy.set_enabled(false).await;
    ingest(&test, &stream_id, "during 1").await;
    ingest(&test, &stream_id, "during 2").await;
    eventually("delivery goes on without Postgres", || async {
        running.egress.versions(running.connection_id) == [1, 2, 3]
    })
    .await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!running.task.is_finished(), "the worker rides out the outage");
    proxy.set_enabled(true).await;

    eventually("persistence resumed", || async {
        test.log
            .head(&stream_id)
            .await
            .map(|h| h.is_some_and(|h| h.version == 3))
            .unwrap_or(false)
    })
    .await;
    running.stop().await;
}

/// Writes commands in rounds of five (the snapshot cadence of the tests) and waits for each snapshot: a worker
/// that is behind stores only the newest one, which would leave the stored snapshots uneven.
async fn write_with_snapshots(test: &Test, stream_id: &StreamId, rounds: i64) {
    for round in 1..=rounds {
        for i in (round - 1) * 5 + 1..=round * 5 {
            ingest(test, stream_id, &format!("m{i}")).await;
        }
        eventually("the next snapshot", || async {
            test.log
                .latest_snapshot(stream_id, round * 5)
                .await
                .unwrap()
                .is_some_and(|snapshot| snapshot.version == round * 5)
        })
        .await;
    }
}

/// A stream with snapshots at 5, 10 and 15 (two kept, so 5 and the commands below 10 are pruned) and its worker.
async fn pruned_stream(test: &Test) -> (StreamId, Running) {
    let stream_id = open(test).await;
    let running = start_worker(test, &stream_id);
    write_with_snapshots(test, &stream_id, 3).await;
    eventually("the log pruned below the oldest kept snapshot", || async {
        test.log.oldest_snapshot_version(&stream_id).await.unwrap() == Some(10)
            && test
                .log
                .commands_after(&stream_id, 0, 100)
                .await
                .unwrap()
                .first()
                .map(|c| c.version)
                == Some(10)
    })
    .await;
    (stream_id, running)
}

#[test]
async fn the_worker_prunes_after_storing_a_snapshot() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let (stream_id, running) = pruned_stream(&test).await;

    // The kept snapshots and the commands from the oldest of them are all there is.
    let versions: Vec<i64> = test
        .log
        .commands_after(&stream_id, 0, 100)
        .await
        .unwrap()
        .iter()
        .map(|c| c.version)
        .collect();
    // The lower bound is already asserted by `pruned_stream`'s wait.
    assert_eq!(versions.last(), Some(&15));
    assert_eq!(test.log.snapshot_horizon(&stream_id, 2).await.unwrap(), Some(10));
    assert_eq!(
        test.log.head(&stream_id).await.unwrap().unwrap().version,
        15,
        "the head is not touched"
    );
    running.stop().await;
}

#[test]
async fn hydration_after_pruning_starts_at_the_oldest_snapshot() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let (stream_id, running) = pruned_stream(&test).await;
    running.stop().await;

    let meta = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert!(test.redis.reset(&stream_id, meta.generation).await.unwrap());
    test.service.ensure_hydrated(&stream_id).await.unwrap();
    assert_eq!(
        redis_versions(&test, &stream_id).await,
        (10..=15).collect::<Vec<_>>(),
        "Redis is rebuilt from what Postgres keeps"
    );

    // A worker starts from the newest snapshot.
    let again = start_worker(&test, &stream_id);
    eventually("the sync", || async {
        !again.egress.sent_to(again.connection_id).is_empty()
    })
    .await;
    let StreamMessage::Sync { snapshot, commands, .. } = again.egress.sent_to(again.connection_id).remove(0) else {
        panic!("a sync first");
    };
    assert_eq!(snapshot.map(|s| s.version), Some(15));
    assert!(commands.is_empty());
    again.stop().await;
}

#[test]
async fn a_resume_below_the_oldest_snapshot_gets_a_full_sync() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let (stream_id, running) = pruned_stream(&test).await;
    eventually("the newest checkpoint", || async {
        running
            .egress
            .checkpoints(running.connection_id)
            .last()
            .map(|(v, _)| *v)
            == Some(15)
    })
    .await;
    let generation = test.redis.meta(&stream_id).await.unwrap().unwrap().generation;

    let other = Uuid::new_v4();
    let resume = StreamResume { generation, version: 3 };
    running
        .handle
        .tx
        .send(WorkerCommand::AddSubscriber(other, Some(resume)))
        .unwrap();
    eventually("the answer", || async { !running.egress.sent_to(other).is_empty() }).await;
    let StreamMessage::Sync { snapshot, .. } = running.egress.sent_to(other).remove(0) else {
        panic!("a position below the history is answered with a sync");
    };
    assert_eq!(snapshot.map(|s| s.version), Some(15));
    running.stop().await;
}

#[test]
async fn the_snapshot_route_after_pruning_returns_the_newest_snapshot_and_its_tail() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let (stream_id, running) = pruned_stream(&test).await;
    ingest(&test, &stream_id, "m16").await;
    ingest(&test, &stream_id, "m17").await;
    eventually("persisted", || async {
        test.log.head(&stream_id).await.unwrap().unwrap().version == 17
    })
    .await;

    let StreamMessage::Sync { snapshot, commands, .. } =
        test.service.read_snapshot(Uuid::nil(), &stream_id).await.unwrap()
    else {
        panic!("a sync");
    };
    assert_eq!(snapshot.map(|s| s.version), Some(15));
    assert_eq!(commands.iter().map(|c| c.version).collect::<Vec<_>>(), [16, 17]);
    running.stop().await;
}

#[test]
async fn a_worker_behind_the_pruned_history_starts_over_from_the_newest_snapshot() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    // A worker that only moves when woken, on the same stores.
    let slow_test = test.with_stores(|stores| {
        stores.settings.sync_interval = Duration::from_secs(60);
        stores.settings.persist_delay = Duration::from_secs(60);
    });
    let stream_id = open(&test).await;
    let slow = start_worker(&slow_test, &stream_id);
    eventually("the initial sync", || async {
        !slow.egress.sent_to(slow.connection_id).is_empty()
    })
    .await;
    ingest(&test, &stream_id, "m1").await;
    slow.handle.wake.notify_one();
    eventually("the first command", || async {
        slow.egress.versions(slow.connection_id) == [1]
    })
    .await;
    let generation = test.redis.meta(&stream_id).await.unwrap().unwrap().generation;

    // The rest of the history moves on without it: snapshots, persistence and the pruning.
    let fast = start_worker(&test, &stream_id);
    write_with_snapshots(&test, &stream_id, 4).await;
    ingest(&test, &stream_id, "last").await; // version 22: the first command above made it 1
    eventually("Redis and Postgres hold only what is above the horizon", || async {
        test.log.oldest_snapshot_version(&stream_id).await.unwrap() == Some(15)
            && redis_versions(&test, &stream_id).await.first() == Some(&15)
    })
    .await;

    slow.handle.wake.notify_one();
    eventually("the slow worker started over", || async {
        // Its own ticks are a minute apart: the reload (after its backoff) needs a push.
        slow.handle.wake.notify_one();
        slow.egress.resets(slow.connection_id) == 1
            && slow.egress.commands(slow.connection_id).last().map(|c| c.version) == Some(22)
    })
    .await;
    let sent = slow.egress.sent_to(slow.connection_id);
    let StreamMessage::Sync { snapshot, .. } = sent.last().unwrap() else {
        panic!("a sync after the reset");
    };
    assert_eq!(snapshot.as_ref().map(|s| s.version), Some(20));
    assert_eq!(
        test.redis.meta(&stream_id).await.unwrap().unwrap().generation,
        generation,
        "the Redis copy was fine and is kept"
    );
    fast.stop().await;
    slow.stop().await;
}

#[test]
async fn a_kind_that_keeps_its_history_is_never_pruned() {
    let Some(test) = setup_with(Arc::new(AllowAll), |config| {
        fast_cadence(config);
        config.kinds.insert(
            "chat".to_string(),
            KindRetentionConfig {
                keep_history: true,
                ..Default::default()
            },
        );
    })
    .await
    else {
        return;
    };
    let stream_id = open(&test).await;
    let running = start_worker(&test, &stream_id);
    // The snapshot at 20 is stored after the ones at 5, 10 and 15 were: a prune would have run by now.
    write_with_snapshots(&test, &stream_id, 4).await;

    let versions: Vec<i64> = test
        .log
        .commands_after(&stream_id, 0, 100)
        .await
        .unwrap()
        .iter()
        .map(|c| c.version)
        .collect();
    assert_eq!(versions.first(), Some(&1));
    assert_eq!(test.log.oldest_snapshot_version(&stream_id).await.unwrap(), Some(5));
    // Redis still trims to its window; the history lives in Postgres.
    ingest(&test, &stream_id, "next").await;
    eventually("Redis trimmed to its window", || async {
        redis_versions(&test, &stream_id).await.first() == Some(&15)
    })
    .await;
    assert_eq!(
        test.log
            .commands_after(&stream_id, 0, 1)
            .await
            .unwrap()
            .first()
            .map(|c| c.version),
        Some(1)
    );
    running.stop().await;
}
