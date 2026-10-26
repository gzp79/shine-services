//! The stream worker on a real Postgres and Redis, with a recording sink instead of the hub. The wake that
//! the inbox listener will give a worker is simulated by calling `wake`. They skip without
//! `SHINE_TEST_PG_CNS` and `SHINE_TEST_REDIS_CNS`.

use crate::{
    models::CommandInboxError,
    repositories::{
        command_inbox::{
            redis::RedisCommandInboxDb,
            tests::test_utils::{create_test_inbox, TestInbox},
            CommandInboxDb, InboxBatch, InboxId, InboxWake,
        },
        command_log::{
            pg::PgCommandLogDb,
            tests::test_utils::{
                all_commands, append_range, create_test_log, replay, stream_id, ChatLog, TestLog, KIND,
            },
            CommandLogDb, StreamId,
        },
    },
    services::command_stream::{
        spawn_worker,
        stream_test_utils::{kinds, quiet, versions_of, wait_finished, TestEgress},
        CommandStreamConfig, CommandStreamService, StreamMessage, WorkerConfig, WorkerHandle,
    },
};
use shine_game::replica::{encode_command, ChatCommand, ChatState};
use shine_test::test;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;
use uuid::Uuid;

struct Harness {
    log: TestLog,
    inbox: TestInbox,
    egress: TestEgress,
}

async fn harness() -> Option<Harness> {
    Some(Harness {
        log: create_test_log().await?,
        inbox: create_test_inbox(60).await?,
        egress: TestEgress::default(),
    })
}

impl Harness {
    /// A worker for the stream, over a service that snapshots every `every` versions.
    fn spawn(&self, stream: &StreamId, every: i64, config: WorkerConfig) -> WorkerHandle {
        self.spawn_with(
            stream,
            CommandStreamConfig {
                snapshot_every: every,
                ..CommandStreamConfig::default()
            },
            config,
        )
    }

    fn spawn_with(&self, stream: &StreamId, service_config: CommandStreamConfig, config: WorkerConfig) -> WorkerHandle {
        let service = CommandStreamService::new(self.inbox.db.clone(), self.log.db.clone(), service_config);
        spawn_worker::<ChatState, _, _, _>(service, self.egress.clone(), stream.clone(), config)
    }

    async fn send(&self, stream: &StreamId, text: &str) {
        let data = encode_command::<ChatState>(&ChatCommand::Add { text: text.to_string() }).unwrap();
        self.inbox
            .db
            .push(stream, Uuid::new_v4(), Uuid::new_v4(), &data)
            .await
            .unwrap();
    }

    /// Subscribes a new connection and waits for its sync.
    async fn subscribe(&self, worker: &WorkerHandle) -> Uuid {
        let connection = Uuid::new_v4();
        assert!(worker.subscribe(connection));
        self.egress.wait(connection, |m| !m.is_empty()).await;
        connection
    }
}

// ---------------------------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_subscriber_is_synced_and_then_gets_the_new_commands_in_order_exactly_once() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 100, quiet());

    let connection = h.subscribe(&worker).await;
    assert_eq!(
        *h.egress.messages(connection)[0],
        StreamMessage::Sync {
            stream_id: id.clone(),
            snapshot: None,
            commands: vec![]
        }
    );
    for text in ["a", "b", "c"] {
        h.send(&id, text).await;
    }
    worker.wake();
    h.egress.wait(connection, |m| versions_of(m, &id) == [1, 2, 3]).await;
    for text in ["d", "e"] {
        h.send(&id, text).await;
    }
    worker.wake();
    h.egress
        .wait(connection, |m| versions_of(m, &id) == [1, 2, 3, 4, 5])
        .await;

    worker.wake();
    sleep(Duration::from_millis(150)).await;
    assert_eq!(versions_of(&h.egress.messages(connection), &id), vec![1, 2, 3, 4, 5]);
}

#[test]
async fn a_backlog_larger_than_one_poll_is_worked_off_without_waiting_for_another_wake() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn_with(
        &id,
        CommandStreamConfig {
            poll_limit: 5,
            ..CommandStreamConfig::default()
        },
        quiet(),
    );
    let connection = h.subscribe(&worker).await;
    for i in 0..12 {
        h.send(&id, &format!("m{i}")).await;
    }

    worker.wake();

    h.egress
        .wait(connection, |m| versions_of(m, &id) == (1..=12).collect::<Vec<_>>())
        .await;
}

#[test]
async fn every_subscriber_of_every_stream_gets_its_own_streams_commands() {
    let Some(h) = harness().await else { return };
    let (a, b) = (stream_id(), stream_id());
    let (worker_a, worker_b) = (h.spawn(&a, 100, quiet()), h.spawn(&b, 100, quiet()));
    let both = h.subscribe(&worker_a).await;
    assert!(worker_b.subscribe(both));
    let only_a = h.subscribe(&worker_a).await;

    h.send(&a, "a1").await;
    h.send(&b, "b1").await;
    h.send(&b, "b2").await;
    worker_a.wake();
    worker_b.wake();

    h.egress
        .wait(both, |m| versions_of(m, &a) == [1] && versions_of(m, &b) == [1, 2])
        .await;
    h.egress.wait(only_a, |m| versions_of(m, &a) == [1]).await;
    assert!(h.egress.messages(only_a).iter().all(|m| m.stream_id() == &a));
}

#[test]
async fn a_late_subscriber_gets_the_snapshot_and_the_tail_and_then_only_newer_commands() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 4, quiet());
    let early = h.subscribe(&worker).await;
    for i in 0..10 {
        h.send(&id, &format!("m{i}")).await;
    }
    worker.wake();
    h.egress.wait(early, |m| versions_of(m, &id).len() == 10).await;

    let late = h.subscribe(&worker).await;

    let commands = all_commands(&h.log.db, &id).await;
    let at_8 = replay(None, &commands[..8]);
    match h.egress.messages(late)[0].as_ref() {
        StreamMessage::Sync {
            snapshot: Some(snapshot),
            commands,
            ..
        } => {
            assert_eq!((snapshot.version, snapshot.state_hash), (8, at_8.hash()));
            assert_eq!(commands.iter().map(|c| c.version).collect::<Vec<_>>(), vec![9, 10]);
        }
        other => panic!("not a sync with a snapshot: {other:?}"),
    }
    h.send(&id, "m10").await;
    worker.wake();
    h.egress.wait(late, |m| versions_of(m, &id) == [9, 10, 11]).await;
}

#[test]
async fn a_checkpoint_is_sent_to_the_subscribers_with_its_state_hash() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 4, quiet());
    let connection = h.subscribe(&worker).await;
    for i in 0..9 {
        h.send(&id, &format!("m{i}")).await;
    }

    worker.wake();
    h.egress.wait(connection, |m| versions_of(m, &id).len() == 9).await;

    let expected = replay(None, &all_commands(&h.log.db, &id).await[..8]).hash();
    h.egress
        .wait(connection, |m| {
            m.iter().any(|message| {
                matches!(message.as_ref(), StreamMessage::Checkpoint { version: 8, state_hash, .. } if *state_hash == expected)
            })
        })
        .await;
}

#[test]
async fn the_backstop_tick_delivers_when_no_wake_comes() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(
        &id,
        100,
        WorkerConfig {
            backstop: Duration::from_millis(50),
            ..quiet()
        },
    );
    let connection = h.subscribe(&worker).await;

    h.send(&id, "a").await;

    h.egress.wait(connection, |m| versions_of(m, &id) == [1]).await;
}

#[test]
async fn a_subscriber_that_cannot_be_sent_to_is_forgotten() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 100, quiet());
    let steady = h.subscribe(&worker).await;
    let flaky = h.subscribe(&worker).await;
    h.send(&id, "a").await;
    worker.wake();
    h.egress.wait(flaky, |m| versions_of(m, &id) == [1]).await;

    h.egress.kill(flaky);
    h.send(&id, "b").await;
    worker.wake();
    h.egress.wait(steady, |m| versions_of(m, &id) == [1, 2]).await;
    h.egress.revive(flaky);
    h.send(&id, "c").await;
    worker.wake();

    h.egress.wait(steady, |m| versions_of(m, &id) == [1, 2, 3]).await;
    assert_eq!(versions_of(&h.egress.messages(flaky), &id), vec![1], "it was dropped");
}

// ---------------------------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------------------------

fn short_grace() -> WorkerConfig {
    WorkerConfig {
        unload_grace: Duration::from_millis(300),
        ..quiet()
    }
}

#[test]
async fn a_subscriber_coming_back_within_the_grace_period_finds_the_stream_loaded() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 100, short_grace());
    let connection = h.subscribe(&worker).await;

    worker.unsubscribe(connection);
    assert!(worker.subscribe(connection));
    h.egress.wait(connection, |m| kinds(m) == ["sync:0", "sync:0"]).await;
    sleep(Duration::from_millis(400)).await;

    assert!(!worker.is_finished(), "the new subscriber keeps the worker");
    h.send(&id, "a").await;
    worker.wake();
    h.egress.wait(connection, |m| versions_of(m, &id) == [1]).await;
}

#[test]
async fn the_worker_ends_when_the_grace_period_after_the_last_subscriber_is_over() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 100, short_grace());
    let connection = h.subscribe(&worker).await;

    worker.unsubscribe(connection);

    wait_finished(&worker).await;
    assert!(
        !worker.subscribe(Uuid::new_v4()),
        "a finished worker takes no subscribers"
    );
}

#[test]
async fn commands_waiting_in_the_inbox_are_persisted_before_the_worker_ends() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let worker = h.spawn(&id, 100, short_grace());
    let connection = h.subscribe(&worker).await;
    h.send(&id, "last words").await;

    worker.unsubscribe(connection);

    wait_finished(&worker).await;
    assert_eq!(all_commands(&h.log.db, &id).await.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Drift and failures
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_log_contradicting_the_worker_resets_the_subscribers_and_syncs_them_again() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    h.log.db.ensure_stream(&id, KIND).await.unwrap();
    append_range(&h.log.db, &id, &ChatLog::new("seed", 3), 1, 3)
        .await
        .unwrap();
    let worker = h.spawn(&id, 100, quiet());
    let connection = h.subscribe(&worker).await;
    // The log is replaced behind the worker's back by another, longer history.
    h.log
        .exec(&format!(
            "UPDATE cl_streams SET version = 0, chain_hash = 0 WHERE stream_id = '{id}';
             DELETE FROM cl_commands WHERE stream_id = '{id}';"
        ))
        .await;
    append_range(&h.log.db, &id, &ChatLog::new("other", 4), 1, 4)
        .await
        .unwrap();
    h.send(&id, "mine").await;

    worker.wake();
    h.egress
        .wait(connection, |m| kinds(m) == ["sync:3", "reset", "sync:4"])
        .await;
    worker.wake();

    h.egress
        .wait(connection, |m| kinds(m) == ["sync:3", "reset", "sync:4", "commands:1"])
        .await;
}

#[test]
async fn a_stream_that_cannot_be_loaded_tells_the_waiting_subscribers_and_ends() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let mut room = ChatLog::new("seed", 3);
    room.commands[1].data = "{not a command".to_string();
    h.log.db.ensure_stream(&id, KIND).await.unwrap();
    append_range(&h.log.db, &id, &room, 1, 3).await.unwrap();
    let worker = h.spawn(&id, 100, quiet());
    let connection = Uuid::new_v4();
    assert!(worker.subscribe(connection));

    h.egress.wait(connection, |m| kinds(m) == ["unsynced"]).await;

    wait_finished(&worker).await;
    assert!(!worker.subscribe(Uuid::new_v4()));
}

/// An inbox that fails to be read when told to.
#[derive(Clone)]
struct FlakyInbox {
    inner: RedisCommandInboxDb,
    fail: Arc<AtomicBool>,
}

impl CommandInboxDb for FlakyInbox {
    async fn push(
        &self,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        data: &str,
    ) -> Result<InboxId, CommandInboxError> {
        self.inner.push(stream_id, command_id, user_id, data).await
    }

    async fn poll(&self, stream_id: &StreamId, after: &InboxId, limit: usize) -> Result<InboxBatch, CommandInboxError> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(CommandInboxError::InvalidConfig("injected failure"));
        }
        self.inner.poll(stream_id, after, limit).await
    }

    async fn trim(&self, stream_id: &StreamId, up_to: &InboxId) -> Result<usize, CommandInboxError> {
        self.inner.trim(stream_id, up_to).await
    }

    async fn notify_persisted(&self, stream_id: &StreamId, version: i64) -> Result<(), CommandInboxError> {
        self.inner.notify_persisted(stream_id, version).await
    }

    async fn listen<F>(&self, handler: F) -> Result<(), CommandInboxError>
    where
        F: Fn(InboxWake) + Send + Sync + 'static,
    {
        self.inner.listen(handler).await
    }

    async fn unlisten(&self) -> Result<(), CommandInboxError> {
        self.inner.unlisten().await
    }
}

#[test]
async fn a_stream_that_keeps_failing_is_given_up_and_its_subscribers_are_told() {
    let Some(h) = harness().await else { return };
    let id = stream_id();
    let fail = Arc::new(AtomicBool::new(false));
    let flaky = FlakyInbox {
        inner: h.inbox.db.clone(),
        fail: fail.clone(),
    };
    let service = CommandStreamService::new(flaky, h.log.db.clone(), CommandStreamConfig::default());
    let worker = spawn_worker::<ChatState, _, _, _>(
        service,
        h.egress.clone(),
        id.clone(),
        WorkerConfig {
            backstop: Duration::from_millis(30),
            max_failures: 2,
            ..quiet()
        },
    );
    let connection = h.subscribe(&worker).await;

    fail.store(true, Ordering::Relaxed);

    h.egress.wait(connection, |m| kinds(m) == ["sync:0", "unsynced"]).await;
    wait_finished(&worker).await;
    assert!(!worker.subscribe(Uuid::new_v4()));
}
