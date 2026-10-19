//! The stream worker: frame ordering without IO, then the whole loop on real Redis and Postgres.

use super::send_frames;
use crate::{
    models::messages::StreamMessage,
    repositories::command_stream::redis::RedisStreamMeta,
    services::command_stream::{
        test_utils::{add, redis_versions, setup_on, setup_with, stored, unique_id, Test},
        AllowAll, IngestOutcome, StreamCore, StreamEgress, WorkerCommand, WorkerExit, WorkerHandle,
    },
    settings::KindSettings,
};
use futures::future::BoxFuture;
use shine_game::replay::ChatState;
use shine_infra::db::command_log::{StoredCommand, StreamId};
use shine_test::{db::get_redis_cns, test, toxiproxy::Toxiproxy};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{task::JoinHandle, time::timeout};
use uuid::Uuid;

/// Records what the workers send; refuses everything to the connections in `refused`.
#[derive(Default)]
struct Recorder {
    refused: HashSet<Uuid>,
    sent: Mutex<Vec<(Uuid, StreamMessage)>>,
}

impl Recorder {
    fn refusing(refused: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            refused: refused.into_iter().collect(),
            ..Default::default()
        }
    }

    fn sent_to(&self, connection_id: Uuid) -> Vec<StreamMessage> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| *id == connection_id)
            .map(|(_, message)| message.clone())
            .collect()
    }

    /// The versions delivered to a connection, in order, `Sync` and `Commands` alike.
    fn versions(&self, connection_id: Uuid) -> Vec<i64> {
        self.sent_to(connection_id)
            .iter()
            .flat_map(|message| match message {
                StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                    commands.iter().map(|c| c.version).collect()
                }
                _ => vec![],
            })
            .collect()
    }

    fn checkpoints(&self, connection_id: Uuid) -> Vec<i64> {
        self.sent_to(connection_id)
            .iter()
            .filter_map(|message| match message {
                StreamMessage::Checkpoint { version, .. } => Some(*version),
                _ => None,
            })
            .collect()
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

#[test]
async fn a_failed_send_stops_all_later_frames() {
    let (fast, slow) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = StreamCore::<ChatState>::new(
        StreamId::parse("chat-test").unwrap(),
        KindSettings {
            snapshot_every: 10_000,
            keep_snapshots: 2,
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

/// Waits until `condition` holds.
async fn eventually<F, Fut>(what: &str, mut condition: F)
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
struct Running {
    handle: WorkerHandle,
    task: JoinHandle<WorkerExit>,
    egress: Arc<Recorder>,
    connection_id: Uuid,
}

fn start_worker(test: &Test, stream_id: &StreamId) -> Running {
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

async fn ingest(test: &Test, stream_id: &StreamId, text: &str) -> IngestOutcome {
    let kind = test.service.kinds().get("chat").unwrap();
    test.service
        .ingest(kind.as_ref(), stream_id, Uuid::new_v4(), Uuid::nil(), &add(text))
        .await
        .unwrap()
}

/// Short cadences: a step every 20 ms, commands persisted right away.
fn fast(config: &mut crate::app_config::CommandStreamConfig) {
    config.sync_interval_ms = 20;
    config.persist_delay_ms = 1;
}

#[test]
async fn worker_persists_commands_and_stores_a_snapshot_every_k() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast).await else {
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
        running.egress.checkpoints(running.connection_id) == vec![5]
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
    let Some(test) = setup_with(Arc::new(AllowAll), fast).await else {
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
    let Some(test) = setup_with(Arc::new(AllowAll), fast).await else {
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
    if get_redis_cns().is_none() {
        return;
    }
    let _reset = toxiproxy.reset_guard().await;
    let proxy = toxiproxy.proxy("redis").await;
    // Short timeouts, so an outage fails fast.
    let cns = format!(
        "redis://{}:{}?timeout=1000&pool_timeout=500",
        toxiproxy.host(),
        proxy.listen_port()
    );
    let Some(test) = setup_on(cns, Arc::new(AllowAll), fast).await else {
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
