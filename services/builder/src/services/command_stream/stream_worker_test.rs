//! The stream worker: frame ordering without IO, then the whole loop on real Redis and Postgres.

use super::send_frames;
use crate::{
    models::messages::StreamMessage,
    repositories::command_stream::redis::RedisStreamMeta,
    services::command_stream::{
        test_utils::{
            add, eventually, fast_cadence, ingest, redis_versions, setup_for_outage, setup_with, start_worker, stored,
            unique_id, Recorder,
        },
        AllowAll, IngestOutcome, StreamCore, WorkerCommand, WorkerExit,
    },
    settings::KindSettings,
};
use shine_game::replay::ChatState;
use shine_infra::db::command_log::{StoredCommand, StreamId};
use shine_test::{
    db::{get_pg_cns, get_redis_cns},
    test,
    toxiproxy::{Stream, Toxic, Toxiproxy},
};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

#[test]
async fn a_failed_send_stops_all_later_frames() {
    let (fast, slow) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = StreamCore::<ChatState>::new(
        StreamId::parse("chat-test").unwrap(),
        KindSettings {
            snapshot_every: 10_000,
            keep_snapshots: 2,
            keep_history: false,
        },
    );
    core.begin_load(None);
    core.finish_load(&RedisStreamMeta {
        generation: 1,
        version: 0,
        persisted: 0,
    });
    core.add_subscriber(fast);
    core.add_subscriber(slow);
    for version in 1..=600 {
        core.apply(stored(version, 1), false).unwrap(); // a Sync and two Commands frames per subscriber
    }

    let egress = Recorder::refusing([slow]);
    let frames = core.take_frames();
    send_frames(&egress, &mut core, frames).await;
    assert_eq!(egress.sent_to(fast).len(), 3);
    assert_eq!(egress.sent_to(slow).len(), 1, "nothing after the first refusal");

    core.apply(stored(601, 1), false).unwrap();
    assert!(
        core.take_frames().iter().all(|(id, _)| *id != slow),
        "a subscriber whose send was refused gets nothing more"
    );
}

#[test]
async fn worker_persists_commands_and_stores_a_snapshot_every_k() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let running = start_worker(&test, &stream_id);

    for i in 1..=6 {
        assert_eq!(
            ingest(&test, &stream_id, &format!("m{i}")).await,
            IngestOutcome::Accepted(i)
        );
    }

    eventually("the commands in Postgres", || async {
        test.log.head(&stream_id).await.unwrap().unwrap().version == 6
    })
    .await;
    eventually("the snapshot at version 5", || async {
        test.log
            .latest_snapshot(&stream_id, 6)
            .await
            .unwrap()
            .is_some_and(|snapshot| snapshot.version == 5)
    })
    .await;
    eventually("the checkpoint and every command", || async {
        running
            .egress
            .checkpoints(running.connection_id)
            .iter()
            .map(|(version, _)| *version)
            .collect::<Vec<_>>()
            == vec![5]
            && running.egress.versions(running.connection_id) == (1..=6).collect::<Vec<_>>()
    })
    .await;
    assert!(
        matches!(
            running.egress.sent_to(running.connection_id).first(),
            Some(StreamMessage::Sync { .. })
        ),
        "the subscriber starts with a Sync"
    );
    let meta = test.redis.meta(&stream_id).await.unwrap().unwrap();
    assert_eq!(meta.persisted, 6, "the persisted version is recorded in Redis");

    running.handle.tx.send(WorkerCommand::Stop).unwrap();
    assert_eq!(running.task.await.unwrap(), WorkerExit::Stopped);
}

#[test]
async fn worker_trims_redis_to_the_snapshot_horizon() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let running = start_worker(&test, &stream_id);

    // Snapshots at 5, 10 and 15 with two kept: the horizon is 10. They are stored one at a time, as a
    // worker that is behind stores only the newest checkpoint.
    for round in 1..=3 {
        for i in (round - 1) * 5 + 1..=round * 5 {
            ingest(&test, &stream_id, &format!("m{i}")).await;
        }
        eventually("the next snapshot", || async {
            test.log
                .latest_snapshot(&stream_id, round * 5)
                .await
                .unwrap()
                .is_some_and(|snapshot| snapshot.version == round * 5)
        })
        .await;
    }

    // The next persisted version trims Redis with the horizon known by then.
    ingest(&test, &stream_id, "m16").await;
    eventually("Redis trimmed to the horizon", || async {
        redis_versions(&test, &stream_id).await == (10..=16).collect::<Vec<_>>()
    })
    .await;

    running.handle.tx.send(WorkerCommand::Stop).unwrap();
    running.task.await.unwrap();
}

#[test]
async fn a_stored_command_that_cannot_be_decoded_stops_the_worker_and_the_stream_is_unavailable() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = unique_id();
    test.log.ensure_stream(&stream_id, "chat").await.unwrap();
    let head = test.log.begin_generation(&stream_id).await.unwrap();
    let broken = StoredCommand {
        data: "not a command".to_string(),
        ..stored(1, head.generation)
    };
    test.log.append(&stream_id, &[broken]).await.unwrap();

    let running = start_worker(&test, &stream_id);
    let exit = timeout(Duration::from_secs(15), running.task).await;
    assert_eq!(exit.unwrap().unwrap(), WorkerExit::Unavailable);
    assert!(
        running.egress.sent_to(running.connection_id).is_empty(),
        "nothing is sent for a stream that cannot be replayed"
    );
}

#[test(serial = "hub-service")]
async fn worker_survives_a_redis_outage_and_recovers() {
    let Some(toxiproxy) = Toxiproxy::from_env() else { return };
    let (Some(_), Some(pg_cns)) = (get_redis_cns(), get_pg_cns()) else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let proxy = toxiproxy.proxy("redis").await;
    // Short timeouts, so an outage fails fast.
    let cns = format!(
        "redis://{}:{}?timeout=1000&pool_timeout=500",
        toxiproxy.host(),
        proxy.listen_port()
    );
    let (Some(test), _outage) = setup_for_outage(cns, pg_cns, Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let running = start_worker(&test, &stream_id);
    ingest(&test, &stream_id, "before").await;
    eventually("the first command", || async {
        running.egress.versions(running.connection_id) == vec![1]
    })
    .await;

    proxy.set_enabled(false).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!running.task.is_finished(), "the worker rides out the outage");
    proxy.set_enabled(true).await;

    // The pool needs a moment to hand out working connections again.
    eventually("a command accepted after the outage", || async {
        let kind = test.service.kinds().get("chat").unwrap();
        matches!(
            test.service
                .ingest(kind.as_ref(), &stream_id, Uuid::new_v4(), Uuid::nil(), &add("after"))
                .await,
            Ok(IngestOutcome::Accepted(_))
        )
    })
    .await;
    eventually("the command after the outage", || async {
        running.egress.versions(running.connection_id) == vec![1, 2]
    })
    .await;
    eventually("everything persisted", || async {
        test.log.head(&stream_id).await.unwrap().unwrap().version == 2
    })
    .await;

    running.handle.tx.send(WorkerCommand::Stop).unwrap();
    running.task.await.unwrap();
}

#[test(serial = "hub-service")]
async fn a_stalled_postgres_query_does_not_freeze_the_worker() {
    let Some(toxiproxy) = Toxiproxy::from_env() else { return };
    let (Some(redis_cns), Some(pg_cns)) = (get_redis_cns(), get_pg_cns()) else {
        return;
    };
    let _reset = toxiproxy.reset_guard().await;
    let proxy = toxiproxy.proxy("postgres").await;
    // A short statement_timeout/tcp_user_timeout, so a stalled query is cut loose quickly;
    // the shipped defaults (10s) would make this test correct but slow. A single-connection pool
    // forces the later appends onto the exact connection "before" already established and
    // returned idle, instead of letting the pool paper over the stall by opening a fresh one.
    let pg_cns = format!("{pg_cns}&options=-c%20statement_timeout=1000&tcp_user_timeout=1&max_size=1");
    let (Some(test), _outage) = setup_for_outage(redis_cns, pg_cns, Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let running = start_worker(&test, &stream_id);
    ingest(&test, &stream_id, "before").await;
    eventually("the first command, persisted", || async {
        test.log
            .head(&stream_id)
            .await
            .unwrap()
            .is_some_and(|head| head.version == 1)
    })
    .await;

    // The worker's pooled connection is already checked out and idle; stall all data on it
    // without closing the TCP connection — a silent network blackhole, or a half-open connection
    // after a failover, looks exactly like this. A long toxic timeout makes sure Postgres itself
    // closing the connection isn't what unblocks the worker. Which of PG-1's session timeout or
    // PG-5's pool-checkout timeout ends up firing first is an implementation detail; either way
    // the worker must recover in a bounded time, not hang for the life of the connection.
    proxy
        .add_toxic(&Toxic::timeout("stall", Stream::Downstream, 60_000))
        .await;
    // Delivery of a command runs before its own persist within one `step()`, so "during" is
    // delivered immediately and only its *persist* gets stuck; a `step()` stuck inside persist
    // never returns, so it is a *later* command ("after-the-stall") that proves the freeze:
    // nothing reads Redis again until that stuck step() call unblocks.
    ingest(&test, &stream_id, "during").await;
    eventually("the second command, delivered", || async {
        running.egress.versions(running.connection_id) == vec![1, 2]
    })
    .await;
    ingest(&test, &stream_id, "after-the-stall").await;

    // The stall genuinely blocks the worker's loop (not just coincidentally slow): the third
    // command is not delivered right away, well before the 1 s statement_timeout could have fired.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        running.egress.versions(running.connection_id),
        vec![1, 2],
        "the stall must still be blocking the worker at this point"
    );

    eventually(
        "the worker keeps delivering once the stalled query is cut loose",
        || async { running.egress.versions(running.connection_id) == vec![1, 2, 3] },
    )
    .await;
    assert!(!running.task.is_finished(), "the worker rides out the stall");

    proxy.remove_toxic("stall").await;
    eventually("everything persisted once Postgres answers again", || async {
        test.log.head(&stream_id).await.unwrap().unwrap().version == 3
    })
    .await;

    running.handle.tx.send(WorkerCommand::Stop).unwrap();
    running.task.await.unwrap();
}
