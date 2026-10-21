//! The stream dispatcher: residency of the stream workers, driven through the hub.

use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, StreamMessage},
    services::{
        command_stream::{
            test_utils::{add, setup_with, unique_id, Test},
            AllowAll, CommandStreamService, StreamEgress, StreamKind, StreamKindRegistry, WorkerCommand, WorkerExit,
            WorkerFuture, WorkerHandle,
        },
        hub_test_utils::{barrier, connect, create_test_hub, wait_event, Egress, TestHub, TestHubOptions},
    },
};
use shine_infra::db::command_log::StreamId;
use shine_test::test;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Notify},
    time::timeout,
};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum Behaviour {
    Run,
    Exits,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Seen {
    Started,
    Added(Uuid),
    Removed(Uuid),
    Stopped,
}

type Log = Arc<Mutex<Vec<(Instant, StreamId, Seen)>>>;

/// A kind whose workers only record what the dispatcher tells them.
struct FakeKind {
    behaviour: Behaviour,
    log: Log,
}

impl StreamKind for FakeKind {
    fn kind(&self) -> &'static str {
        "fake"
    }

    fn canonicalize(&self, command: &str) -> Option<String> {
        Some(command.to_string())
    }

    fn create_worker(
        &self,
        _egress: Arc<dyn StreamEgress>,
        _streams: CommandStreamService,
        stream_id: StreamId,
    ) -> (WorkerHandle, WorkerFuture) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (log, behaviour) = (self.log.clone(), self.behaviour);
        let record = move |seen| log.lock().unwrap().push((Instant::now(), stream_id.clone(), seen));
        let future = Box::pin(async move {
            record(Seen::Started);
            match behaviour {
                Behaviour::Exits => return WorkerExit::Stopped,
                Behaviour::Unavailable => return WorkerExit::Unavailable,
                Behaviour::Run => {}
            }
            while let Some(command) = rx.recv().await {
                match command {
                    WorkerCommand::AddSubscriber(connection_id, _) => record(Seen::Added(connection_id)),
                    WorkerCommand::RemoveSubscriber(connection_id) => record(Seen::Removed(connection_id)),
                    WorkerCommand::Stop => {
                        record(Seen::Stopped);
                        break;
                    }
                }
            }
            WorkerExit::Stopped
        });
        (
            WorkerHandle {
                tx,
                wake: Arc::new(Notify::new()),
            },
            future,
        )
    }
}

fn fast(config: &mut crate::app_config::CommandStreamConfig) {
    config.sync_interval_ms = 20;
    config.persist_delay_ms = 1;
    config.unload_grace_seconds = 0;
}

/// A hub whose command streams have the fake kind, on top of the real stores.
async fn fake_hub(behaviour: Behaviour) -> Option<(TestHub, Log)> {
    let streams: Test = setup_with(Arc::new(AllowAll), fast).await?;
    let log = Log::default();
    let kinds = StreamKindRegistry::new().with_kind(Arc::new(FakeKind { behaviour, log: log.clone() }));
    let service = CommandStreamService::new(streams.service.stores().clone(), kinds, Arc::new(AllowAll));
    let hub = create_test_hub(TestHubOptions {
        command_streams: Some(service),
        ..Default::default()
    })
    .await?;
    Some((hub, log))
}

fn seen(log: &Log, stream_id: &StreamId) -> Vec<Seen> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, id, _)| id == stream_id)
        .map(|(_, _, seen)| seen.clone())
        .collect()
}

async fn eventually_seen(log: &Log, stream_id: &StreamId, expected: Vec<Seen>) {
    let found = timeout(Duration::from_secs(10), async {
        while seen(log, stream_id) != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "expected {expected:?}, saw {:?}", seen(log, stream_id));
}

async fn next_stream_message(egress: &mut Egress) -> StreamMessage {
    timeout(Duration::from_secs(10), async {
        loop {
            if let Some(HubMessage::Stream(message)) = egress.recv().await {
                return message;
            }
        }
    })
    .await
    .expect("no stream message arrived")
}

#[test(serial = "hub-service")]
async fn the_dispatcher_drops_the_streams_of_a_disconnected_connection() {
    let Some((mut test, log)) = fake_hub(Behaviour::Run).await else {
        return;
    };
    let (user_id, stream_id) = (Uuid::new_v4(), unique_id());
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "fake", None)
        .unwrap();
    barrier(&test.hub, &mut test.events).await;
    eventually_seen(&log, &stream_id, vec![Seen::Started, Seen::Added(connection_id)]).await;

    test.hub.request_disconnection(user_id, connection_id).unwrap();
    barrier(&test.hub, &mut test.events).await;
    eventually_seen(
        &log,
        &stream_id,
        vec![
            Seen::Started,
            Seen::Added(connection_id),
            Seen::Removed(connection_id),
            Seen::Stopped,
        ],
    )
    .await;
}

#[test(serial = "hub-service")]
async fn a_stream_without_subscribers_is_unloaded_after_the_grace_time() {
    let Some((mut test, log)) = fake_hub(Behaviour::Run).await else {
        return;
    };
    let (user_id, stream_id) = (Uuid::new_v4(), unique_id());
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "fake", None)
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    // Several ticks pass while it has a subscriber.
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !seen(&log, &stream_id).contains(&Seen::Stopped),
        "a stream with a subscriber stays loaded"
    );

    test.hub
        .request_unsubscribe_stream(user_id, connection_id, stream_id.clone())
        .unwrap();
    barrier(&test.hub, &mut test.events).await;
    eventually_seen(
        &log,
        &stream_id,
        vec![
            Seen::Started,
            Seen::Added(connection_id),
            Seen::Removed(connection_id),
            Seen::Stopped,
        ],
    )
    .await;
}

#[test(serial = "hub-service")]
async fn a_crashed_worker_is_restarted_once_per_respawn_delay() {
    let Some((mut test, log)) = fake_hub(Behaviour::Exits).await else {
        return;
    };
    let (user_id, stream_id) = (Uuid::new_v4(), unique_id());
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "fake", None)
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    tokio::time::sleep(Duration::from_millis(2500)).await;
    let starts: Vec<Instant> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, id, seen)| *id == stream_id && *seen == Seen::Started)
        .map(|(at, _, _)| *at)
        .collect();
    assert!(
        (2..=4).contains(&starts.len()),
        "restarted, but not in a loop: {} starts",
        starts.len()
    );
    for pair in starts.windows(2) {
        assert!(
            pair[1] - pair[0] >= Duration::from_millis(900),
            "a restart came too early"
        );
    }
}

#[test(serial = "hub-service")]
async fn a_stream_that_cannot_be_replayed_refuses_its_subscribers() {
    let Some((mut test, _log)) = fake_hub(Behaviour::Unavailable).await else {
        return;
    };
    let (user_id, stream_id) = (Uuid::new_v4(), unique_id());
    let (connection_id, mut egress) = connect(&test.hub, user_id, 16);

    for _ in 0..2 {
        test.hub
            .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "fake", None)
            .unwrap();
        barrier(&test.hub, &mut test.events).await;
        assert_eq!(
            next_stream_message(&mut egress).await,
            StreamMessage::SubscribeFailed {
                stream_id: stream_id.clone(),
                reason: "unavailable"
            }
        );
    }
}

#[test(serial = "hub-service")]
async fn a_slow_subscriber_never_sees_a_version_gap() {
    let Some(streams) = setup_with(Arc::new(AllowAll), fast).await else {
        return;
    };
    let (user_id, stream_id) = (Uuid::new_v4(), unique_id());
    let opened = streams.service.open(user_id, &stream_id, "chat").await.unwrap();
    let Some(mut test) = create_test_hub(TestHubOptions {
        command_streams: Some(streams.service.clone()),
        ..Default::default()
    })
    .await
    else {
        return;
    };
    // A channel for two messages that nobody reads.
    let (connection_id, mut egress) = connect(&test.hub, user_id, 2);
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "chat", None)
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    for i in 1..=60 {
        streams
            .service
            .ingest(
                opened.kind.as_ref(),
                &stream_id,
                Uuid::new_v4(),
                user_id,
                &add(&format!("m{i}")),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let dropped = wait_event(&mut test.events, Duration::from_secs(15), |event| {
        matches!(
            event,
            HubEvent::UserDisconnected {
                reason: DisconnectReason::SlowConsumer,
                ..
            }
        )
    })
    .await;
    assert!(dropped.is_some(), "the hub drops a connection that cannot keep up");

    // What it did receive is a contiguous run, and no checkpoint is ahead of it.
    let mut versions = vec![];
    let mut checkpoints = vec![];
    while let Some(message) = timeout(Duration::from_secs(5), egress.recv()).await.unwrap() {
        match message {
            HubMessage::Stream(StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. }) => {
                versions.extend(commands.iter().map(|c| c.version))
            }
            HubMessage::Stream(StreamMessage::Checkpoint { version, .. }) => checkpoints.push(version),
            _ => {}
        }
    }
    assert!(versions.len() < 60, "it was dropped before the end");
    assert!(
        versions.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "a gap in the delivered versions: {versions:?}"
    );
    let last = versions.last().copied().unwrap_or(0);
    assert!(
        checkpoints.iter().all(|version| *version <= last),
        "a checkpoint ahead of the commands"
    );
}

#[test(serial = "hub-service")]
async fn subscribe_unsubscribe_churn_leaves_no_subscription() {
    let Some((mut test, log)) = fake_hub(Behaviour::Run).await else {
        return;
    };
    let user_id = Uuid::new_v4();
    let streams: Vec<StreamId> = (0..3).map(|_| unique_id()).collect();
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);

    for cycle in 0..500 {
        let stream_id = streams[cycle % streams.len()].clone();
        test.hub
            .request_subscribe_stream(user_id, connection_id, stream_id.clone(), "fake", None)
            .unwrap();
        test.hub
            .request_unsubscribe_stream(user_id, connection_id, stream_id)
            .unwrap();
    }
    barrier(&test.hub, &mut test.events).await;

    let settled = timeout(Duration::from_secs(15), async {
        loop {
            let balanced = streams.iter().all(|stream_id| {
                let seen = seen(&log, stream_id);
                let count = |wanted: fn(&Seen) -> bool| seen.iter().filter(|s| wanted(s)).count();
                count(|s| matches!(s, Seen::Added(_))) == count(|s| matches!(s, Seen::Removed(_)))
                    && count(|s| *s == Seen::Started) == count(|s| *s == Seen::Stopped)
                    && seen.last() == Some(&Seen::Stopped)
            });
            if balanced {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(
        settled.is_ok(),
        "a subscription or a worker was leaked: {:?}",
        log.lock().unwrap().len()
    );
}
