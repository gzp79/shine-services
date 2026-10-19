//! Several workers and instances on real Redis and Postgres: ordering, snapshot disagreement, losing or
//! rolling back Redis, and a Postgres outage.

use crate::{
    models::messages::StreamMessage,
    repositories::command_stream::redis::RedisStreamEntry,
    services::command_stream::{
        test_utils::{
            add, eventually, fast_cadence, ingest, setup_for_outage, setup_with, start_worker, unique_id, Running, Test,
        },
        AllowAll, IngestOutcome,
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
    let stored: Vec<Uuid> = first
        .log
        .commands_after(&stream_id, 0, 100)
        .await
        .unwrap()
        .iter()
        .map(|c| c.command_id)
        .collect();
    assert_eq!(
        stored,
        command_ids(&worker_a),
        "Postgres holds the delivered order, without duplicates"
    );

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
