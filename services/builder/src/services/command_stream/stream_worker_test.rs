//! The stream worker on a real Postgres and Redis, with a recording sink instead of the hub. The wake that
//! the inbox listener will give a worker is simulated by calling `wake`.

use crate::{
    models::{messages::StreamMessage, CommandStreamError},
    repositories::{
        command_inbox::{redis::RedisCommandInboxDb, CommandInboxDb, InboxBatch, InboxId, InboxWake},
        command_log::{pg::PgCommandLogDb, CommandLogDb, ConsistencyReport, StoredCommand, StoredSnapshot, StreamHead},
    },
    services::command_stream::{InboxConfig, LogConfig, StreamConfig, StreamLogService, StreamWorker},
    testing::{
        command_stream::{ChatLog, TestEgress, TestInbox, TestLog, TestStreamId},
        TestMessage,
    },
};
use shine_game::replica::{encode_command, ChatCommand, ChatState, StreamId};
use shine_test::test;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn wait_finished(worker: &StreamWorker) {
    let result = timeout(WAIT_TIMEOUT, worker.wait_finished()).await;
    assert!(result.is_ok(), "the worker did not end");
}

struct Harness {
    log: TestLog,
    inbox: TestInbox,
    egress: TestEgress,
}

async fn harness() -> Harness {
    Harness {
        log: TestLog::from_env().await,
        inbox: TestInbox::from_env().await,
        egress: TestEgress::default(),
    }
}

impl Harness {
    /// A worker for the stream, over a service that snapshots every `every` versions.
    fn spawn(&self, stream: &StreamId, every: i64, mut config: StreamConfig) -> Arc<StreamWorker> {
        config.log.snapshot_every = every;
        self.spawn_with(stream, config)
    }

    fn spawn_with(&self, stream: &StreamId, config: StreamConfig) -> Arc<StreamWorker> {
        let service = StreamLogService::new(self.inbox.db.clone(), self.log.db.clone(), config.clone());
        StreamWorker::start::<ChatState, _, _, _>(
            service,
            self.inbox.db.clone(),
            self.egress.clone(),
            stream.clone(),
            config,
        )
    }

    async fn send(&self, stream: &StreamId, text: &str) {
        let data = TestMessage::new_chat_command(text);
        self.inbox
            .db
            .push(stream, Uuid::new_v4(), Uuid::new_v4(), &data)
            .await
            .unwrap();
    }

    /// Subscribes a new connection and waits for its sync.
    async fn subscribe(&self, worker: &StreamWorker) -> Uuid {
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
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());

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
    h.egress
        .wait(connection, |m| TestEgress::versions(m, &id) == [1, 2, 3])
        .await;
    for text in ["d", "e"] {
        h.send(&id, text).await;
    }
    worker.wake();
    h.egress
        .wait(connection, |m| TestEgress::versions(m, &id) == [1, 2, 3, 4, 5])
        .await;

    worker.wake();
    sleep(Duration::from_millis(150)).await;
    assert_eq!(
        TestEgress::versions(&h.egress.messages(connection), &id),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
async fn a_backlog_larger_than_one_poll_is_worked_off_without_waiting_for_another_wake() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn_with(
        &id,
        StreamConfig {
            inbox: InboxConfig {
                poll_limit: 5,
                ..StreamConfig::test_default().inbox
            },
            ..StreamConfig::test_default()
        },
    );
    let connection = h.subscribe(&worker).await;
    for i in 0..12 {
        h.send(&id, &format!("m{i}")).await;
    }

    worker.wake();

    h.egress
        .wait(connection, |m| {
            TestEgress::versions(m, &id) == (1..=12).collect::<Vec<_>>()
        })
        .await;
}

#[test]
async fn a_late_subscriber_gets_the_snapshot_and_the_tail_and_then_only_newer_commands() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 4, StreamConfig::test_default());
    let early = h.subscribe(&worker).await;
    for i in 0..10 {
        h.send(&id, &format!("m{i}")).await;
    }
    worker.wake();
    h.egress.wait(early, |m| TestEgress::versions(m, &id).len() == 10).await;
    // The snapshot at 8 reaches the worker with the answer of the persistence.
    let late = timeout(WAIT_TIMEOUT, async {
        loop {
            let connection = h.subscribe(&worker).await;
            if matches!(
                h.egress.messages(connection)[0].as_ref(),
                StreamMessage::Sync { snapshot: Some(_), .. }
            ) {
                return connection;
            }
            worker.unsubscribe(connection);
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a sync with a snapshot");

    let commands = h.log.all_commands(&id).await;
    let at_8 = ChatLog::replay(None, &commands[..8]);
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
    h.egress
        .wait(late, |m| TestEgress::versions(m, &id) == [9, 10, 11])
        .await;
}

#[test]
async fn the_recovery_poll_delivers_when_no_wake_comes() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(
        &id,
        100,
        StreamConfig {
            inbox: InboxConfig {
                recovery_poll_interval: Duration::from_millis(50),
                ..StreamConfig::test_default().inbox
            },
            ..StreamConfig::test_default()
        },
    );
    let connection = h.subscribe(&worker).await;

    h.send(&id, "a").await;

    h.egress.wait(connection, |m| TestEgress::versions(m, &id) == [1]).await;
}

#[test]
async fn a_subscriber_that_cannot_be_sent_to_is_forgotten() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    let steady = h.subscribe(&worker).await;
    let flaky = h.subscribe(&worker).await;
    h.send(&id, "a").await;
    worker.wake();
    h.egress.wait(flaky, |m| TestEgress::versions(m, &id) == [1]).await;

    h.egress.kill(flaky);
    h.send(&id, "b").await;
    worker.wake();
    h.egress.wait(steady, |m| TestEgress::versions(m, &id) == [1, 2]).await;
    h.egress.revive(flaky);
    h.send(&id, "c").await;
    worker.wake();

    h.egress
        .wait(steady, |m| TestEgress::versions(m, &id) == [1, 2, 3])
        .await;
    assert_eq!(
        TestEgress::versions(&h.egress.messages(flaky), &id),
        vec![1],
        "it was dropped"
    );
}

// ---------------------------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------------------------

fn short_grace_config() -> StreamConfig {
    StreamConfig {
        worker_unload_grace: Duration::from_millis(300),
        ..StreamConfig::test_default()
    }
}

#[test]
async fn a_subscriber_coming_back_within_the_grace_period_finds_the_stream_loaded() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, short_grace_config());
    let connection = h.subscribe(&worker).await;

    worker.unsubscribe(connection);
    assert!(worker.subscribe(connection));
    h.egress
        .wait(connection, |m| TestEgress::kinds(m) == ["sync:0", "sync:0"])
        .await;
    sleep(Duration::from_millis(400)).await;

    assert!(!worker.is_finished(), "the new subscriber keeps the worker");
    h.send(&id, "a").await;
    worker.wake();
    h.egress.wait(connection, |m| TestEgress::versions(m, &id) == [1]).await;
}

#[test]
async fn the_worker_ends_when_the_grace_period_after_the_last_subscriber_is_over() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, short_grace_config());
    let connection = h.subscribe(&worker).await;

    worker.unsubscribe(connection);

    wait_finished(&worker).await;
    assert!(
        !worker.subscribe(Uuid::new_v4()),
        "a finished worker takes no subscribers"
    );
}

#[test]
async fn a_worker_nobody_subscribes_to_ends_after_the_grace_period() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, short_grace_config());

    wait_finished(&worker).await;
}

#[test]
async fn commands_waiting_in_the_inbox_are_persisted_before_the_worker_unloads() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, short_grace_config());
    let connection = h.subscribe(&worker).await;
    h.send(&id, "last words").await;

    worker.unsubscribe(connection);

    wait_finished(&worker).await;
    assert_eq!(h.log.all_commands(&id).await.len(), 1);
}

#[test]
async fn a_subscriber_queued_while_the_worker_stops_is_not_served() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    h.subscribe(&worker).await;
    let late = Uuid::new_v4();

    assert!(worker.stop());
    worker.subscribe(late);

    wait_finished(&worker).await;
    assert!(h.egress.messages(late).is_empty());
    assert!(h.egress.failed().is_empty(), "stopping is not a stream failure");
}

#[test]
async fn a_stopped_worker_ends_and_takes_no_more_subscribers() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    h.subscribe(&worker).await;

    assert!(worker.stop());

    wait_finished(&worker).await;
    assert!(
        !worker.subscribe(Uuid::new_v4()),
        "a finished worker takes no subscribers"
    );
}

#[test]
async fn commands_waiting_in_the_inbox_are_persisted_before_the_worker_ends() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    h.subscribe(&worker).await;
    h.send(&id, "last words").await;

    worker.stop();

    wait_finished(&worker).await;
    assert_eq!(h.log.all_commands(&id).await.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Drift and failures
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_log_contradicting_the_worker_resets_the_subscribers_and_syncs_them_again() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    h.log.db.ensure_stream(&id).await.unwrap();
    ChatLog::new("seed", 3).persist(&h.log.db, &id, 1, 3).await.unwrap();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    let connection = h.subscribe(&worker).await;
    // The log is replaced behind the worker's back by another, longer history.
    h.log
        .exec(&format!(
            "UPDATE cl_streams SET version = 0, chain_hash = 0 WHERE kind = '{}' AND id = '{}';
             DELETE FROM cl_commands WHERE kind = '{}' AND id = '{}';",
            id.kind(),
            id.id(),
            id.kind(),
            id.id(),
        ))
        .await;
    ChatLog::new("other", 4).persist(&h.log.db, &id, 1, 4).await.unwrap();
    h.send(&id, "mine").await;

    worker.wake();
    h.egress
        .wait(connection, |m| {
            TestEgress::kinds(m) == ["sync:3", "commands:1", "sync:5"]
        })
        .await;
    let messages = h.egress.messages(connection);
    let StreamMessage::Sync { commands, .. } = &*messages[2] else {
        panic!("expected a corrected sync");
    };
    assert_eq!(commands.last().unwrap().version, 5);
    assert_eq!(commands.last().unwrap().command, TestMessage::new_chat_command("mine"));
}

#[test]
async fn a_stream_that_cannot_be_loaded_tells_the_waiting_subscribers_and_ends() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let mut room = ChatLog::new("seed", 3);
    room.commands[1].data = b"{not a command".to_vec();
    h.log.db.ensure_stream(&id).await.unwrap();
    room.persist(&h.log.db, &id, 1, 3).await.unwrap();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    let connection = Uuid::new_v4();
    assert!(worker.subscribe(connection));

    wait_finished(&worker).await;
    assert_eq!(h.egress.failed(), [id]);
    assert!(h.egress.messages(connection).is_empty());
    assert!(!worker.subscribe(Uuid::new_v4()));
}

/// An inbox that fails the next reads when told to: `fail` is how many. The failure is one that may pass
/// (an outage) if `transient`, a verdict about the data otherwise.
#[derive(Clone)]
struct FlakyInbox {
    inner: RedisCommandInboxDb,
    fail: Arc<AtomicUsize>,
    transient: bool,
}

impl CommandInboxDb for FlakyInbox {
    async fn push(
        &self,
        stream_id: &StreamId,
        command_id: Uuid,
        user_id: Uuid,
        data: &[u8],
    ) -> Result<InboxId, CommandStreamError> {
        self.inner.push(stream_id, command_id, user_id, data).await
    }

    async fn poll(
        &self,
        stream_id: &StreamId,
        after: &InboxId,
        limit: usize,
    ) -> Result<InboxBatch, CommandStreamError> {
        let failing = self
            .fail
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |left| left.checked_sub(1))
            .is_ok();
        if failing {
            return Err(if self.transient {
                CommandStreamError::service_unavailable("Redis", std::io::Error::other("injected failure"))
            } else {
                CommandStreamError::Corrupt("injected failure".to_string())
            });
        }
        self.inner.poll(stream_id, after, limit).await
    }

    async fn trim(&self, stream_id: &StreamId, up_to: &InboxId) -> Result<usize, CommandStreamError> {
        self.inner.trim(stream_id, up_to).await
    }

    async fn len(&self, stream_id: &StreamId) -> Result<usize, CommandStreamError> {
        self.inner.len(stream_id).await
    }

    async fn notify_persisted(&self, stream_id: &StreamId, version: i64) -> Result<(), CommandStreamError> {
        self.inner.notify_persisted(stream_id, version).await
    }

    async fn listen<F>(&self, handler: F) -> Result<(), CommandStreamError>
    where
        F: Fn(InboxWake) + Send + Sync + 'static,
    {
        self.inner.listen(handler).await
    }

    async fn unlisten(&self) -> Result<(), CommandStreamError> {
        self.inner.unlisten().await
    }
}

#[test]
async fn a_stream_that_fails_is_given_up_and_its_subscribers_are_told() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let fail = Arc::new(AtomicUsize::new(0));
    let flaky = FlakyInbox {
        inner: h.inbox.db.clone(),
        fail: fail.clone(),
        transient: false,
    };
    let config = StreamConfig {
        inbox: InboxConfig {
            recovery_poll_interval: Duration::from_millis(30),
            ..StreamConfig::test_default().inbox
        },
        ..StreamConfig::test_default()
    };
    let service = StreamLogService::new(flaky.clone(), h.log.db.clone(), config.clone());
    let worker = StreamWorker::start::<ChatState, _, _, _>(service, flaky, h.egress.clone(), id.clone(), config);
    let connection = h.subscribe(&worker).await;

    fail.store(usize::MAX, Ordering::Relaxed);

    wait_finished(&worker).await;
    assert_eq!(h.egress.failed(), [id]);
    assert_eq!(TestEgress::kinds(&h.egress.messages(connection)), ["sync:0"]);
    assert!(!worker.subscribe(Uuid::new_v4()));
}

#[test]
async fn a_subscriber_is_told_to_retry_without_a_sync_when_the_inbox_read_fails() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let fail = Arc::new(AtomicUsize::new(0));
    let flaky = FlakyInbox {
        inner: h.inbox.db.clone(),
        fail: fail.clone(),
        transient: false,
    };
    let config = StreamConfig::test_default();
    let service = StreamLogService::new(flaky.clone(), h.log.db.clone(), config.clone());
    let worker = StreamWorker::start::<ChatState, _, _, _>(service, flaky, h.egress.clone(), id.clone(), config);
    h.subscribe(&worker).await;
    fail.store(usize::MAX, Ordering::Relaxed);
    let connection = Uuid::new_v4();

    assert!(worker.subscribe(connection));

    wait_finished(&worker).await;
    assert_eq!(h.egress.failed(), [id]);
    assert!(h.egress.messages(connection).is_empty());
}

#[test]
async fn an_inbox_that_fails_for_a_moment_does_not_end_the_stream() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let fail = Arc::new(AtomicUsize::new(0));
    let flaky = FlakyInbox {
        inner: h.inbox.db.clone(),
        fail: fail.clone(),
        transient: true,
    };
    let config = StreamConfig::test_default();
    let service = StreamLogService::new(flaky.clone(), h.log.db.clone(), config.clone());
    let worker = StreamWorker::start::<ChatState, _, _, _>(service, flaky, h.egress.clone(), id.clone(), config);
    let connection = h.subscribe(&worker).await;

    fail.store(3, Ordering::Relaxed);
    h.send(&id, "a").await;
    worker.wake();

    h.egress.wait(connection, |m| TestEgress::versions(m, &id) == [1]).await;
    assert_eq!(fail.load(Ordering::Relaxed), 0, "the failing reads were all spent");
    assert!(!worker.is_finished());
    assert!(h.egress.failed().is_empty(), "the subscriber was not dropped");
}

/// How many times the inbox is read until a worker over an inbox that always fails (an outage) gives up.
async fn reads_until_given_up(h: &Harness, max_retries: u32) -> usize {
    let id = TestStreamId::new_chat();
    let fail = Arc::new(AtomicUsize::new(0));
    let flaky = FlakyInbox {
        inner: h.inbox.db.clone(),
        fail: fail.clone(),
        transient: true,
    };
    let config = StreamConfig {
        inbox: InboxConfig {
            max_retries,
            ..StreamConfig::test_default().inbox
        },
        ..StreamConfig::test_default()
    };
    let service = StreamLogService::new(flaky.clone(), h.log.db.clone(), config.clone());
    let worker = StreamWorker::start::<ChatState, _, _, _>(service, flaky, h.egress.clone(), id.clone(), config);
    h.subscribe(&worker).await;
    fail.store(usize::MAX, Ordering::Relaxed);
    let connection = Uuid::new_v4();

    assert!(worker.subscribe(connection));

    wait_finished(&worker).await;
    assert_eq!(h.egress.failed(), [id]);
    assert!(h.egress.messages(connection).is_empty());
    usize::MAX - fail.load(Ordering::Relaxed)
}

#[test]
async fn an_inbox_that_keeps_failing_ends_the_stream_once_the_retries_are_spent() {
    let h = harness().await;
    let retries = StreamConfig::test_default().inbox.max_retries;

    let reads = reads_until_given_up(&h, retries).await;

    assert_eq!(reads, retries as usize + 1, "the first read and every retry");
}

#[test]
async fn a_stream_without_retries_gives_up_at_the_first_failed_read() {
    let h = harness().await;

    assert_eq!(reads_until_given_up(&h, 0).await, 1);
}

/// The worker that ends while the log holds a history of its own leaves the commands it could not store
/// in the inbox, for whoever serves the stream next.
#[test]
async fn a_worker_stopped_over_a_contradicting_log_leaves_its_commands_in_the_inbox() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    h.log.db.ensure_stream(&id).await.unwrap();
    ChatLog::new("seed", 3).persist(&h.log.db, &id, 1, 3).await.unwrap();
    let worker = h.spawn(&id, 100, StreamConfig::test_default());
    h.subscribe(&worker).await;
    h.log
        .exec(&format!(
            "UPDATE cl_streams SET version = 0, chain_hash = 0 WHERE kind = '{}' AND id = '{}';
             DELETE FROM cl_commands WHERE kind = '{}' AND id = '{}';",
            id.kind(),
            id.id(),
            id.kind(),
            id.id(),
        ))
        .await;
    ChatLog::new("other", 4).persist(&h.log.db, &id, 1, 4).await.unwrap();
    h.send(&id, "mine").await;

    assert!(worker.stop());

    wait_finished(&worker).await;
    assert_eq!(h.log.all_commands(&id).await.len(), 4, "the log keeps its own history");
    assert_eq!(h.inbox.len(&id).await, 1, "the command was not lost");
}

/// A command log that fails the next calls when told to, as a database outage does: `fail_append` is how many
/// appends and `fail_read` how many loads of the stream's head and history.
#[derive(Clone)]
struct FlakyLog {
    inner: PgCommandLogDb,
    fail_append: Arc<AtomicUsize>,
    fail_read: Arc<AtomicUsize>,
}

impl FlakyLog {
    fn new(inner: PgCommandLogDb) -> Self {
        Self {
            inner,
            fail_append: Arc::new(AtomicUsize::new(0)),
            fail_read: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn outage(counter: &AtomicUsize) -> Result<(), CommandStreamError> {
        let failing = counter
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |left| left.checked_sub(1))
            .is_ok();
        if failing {
            return Err(CommandStreamError::service_unavailable(
                "PostgreSQL",
                std::io::Error::other("injected failure"),
            ));
        }
        Ok(())
    }
}

impl CommandLogDb for FlakyLog {
    async fn ensure_stream(&self, stream_id: &StreamId) -> Result<StreamHead, CommandStreamError> {
        Self::outage(&self.fail_read)?;
        self.inner.ensure_stream(stream_id).await
    }

    async fn head(&self, stream_id: &StreamId) -> Result<Option<StreamHead>, CommandStreamError> {
        Self::outage(&self.fail_read)?;
        self.inner.head(stream_id).await
    }

    async fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
        baseline_hash: u64,
    ) -> Result<StreamHead, CommandStreamError> {
        Self::outage(&self.fail_append)?;
        self.inner.append(stream_id, commands, baseline_hash).await
    }

    async fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> Result<Vec<StoredCommand>, CommandStreamError> {
        self.inner.commands_after(stream_id, after, limit).await
    }

    async fn last_commands(&self, stream_id: &StreamId, count: i64) -> Result<Vec<StoredCommand>, CommandStreamError> {
        self.inner.last_commands(stream_id, count).await
    }

    async fn try_store_snapshot(
        &self,
        stream_id: &StreamId,
        snapshot: &StoredSnapshot,
    ) -> Result<(), CommandStreamError> {
        self.inner.try_store_snapshot(stream_id, snapshot).await
    }

    async fn latest_snapshot(
        &self,
        stream_id: &StreamId,
        at_or_before: i64,
    ) -> Result<Option<StoredSnapshot>, CommandStreamError> {
        self.inner.latest_snapshot(stream_id, at_or_before).await
    }

    async fn oldest_snapshot_version(&self, stream_id: &StreamId) -> Result<Option<i64>, CommandStreamError> {
        self.inner.oldest_snapshot_version(stream_id).await
    }

    async fn prune(&self, stream_id: &StreamId, kept_snapshot_count: i64) -> Result<Option<i64>, CommandStreamError> {
        self.inner.prune(stream_id, kept_snapshot_count).await
    }

    async fn check_consistency(&self, stream_id: &StreamId) -> Result<ConsistencyReport, CommandStreamError> {
        self.inner.check_consistency(stream_id).await
    }
}

impl Harness {
    /// A worker whose command log is flaky.
    fn spawn_over_flaky_log(&self, stream: &StreamId, log: FlakyLog) -> Arc<StreamWorker> {
        self.spawn_over_flaky_log_with(stream, log, StreamConfig::test_default())
    }

    fn spawn_over_flaky_log_with(&self, stream: &StreamId, log: FlakyLog, config: StreamConfig) -> Arc<StreamWorker> {
        let service = StreamLogService::new(self.inbox.db.clone(), log, config.clone());
        StreamWorker::start::<ChatState, _, _, _>(
            service,
            self.inbox.db.clone(),
            self.egress.clone(),
            stream.clone(),
            config,
        )
    }
}

#[test]
async fn a_log_that_fails_for_a_moment_does_not_end_the_stream() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let log = FlakyLog::new(h.log.db.clone());
    let worker = h.spawn_over_flaky_log(&id, log.clone());
    let connection = h.subscribe(&worker).await;

    log.fail_append.store(3, Ordering::Relaxed);
    h.send(&id, "a").await;
    worker.wake();

    h.log.wait_commands(&id, 1).await;
    assert_eq!(
        log.fail_append.load(Ordering::Relaxed),
        0,
        "the failing appends were all spent"
    );
    assert!(!worker.is_finished());
    assert!(h.egress.failed().is_empty(), "the subscriber was not dropped");
}

#[test]
async fn a_log_that_fails_for_a_moment_while_loading_does_not_end_the_stream() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let log = FlakyLog::new(h.log.db.clone());
    log.fail_read.store(3, Ordering::Relaxed);
    let worker = h.spawn_over_flaky_log(&id, log.clone());
    let connection = Uuid::new_v4();

    assert!(worker.subscribe(connection));

    h.egress.wait(connection, |m| !m.is_empty()).await;
    assert_eq!(TestEgress::kinds(&h.egress.messages(connection)), ["sync:0"]);
    assert!(!worker.is_finished());
}

#[test]
async fn a_log_that_keeps_failing_ends_the_stream_once_the_retries_are_spent() {
    let h = harness().await;
    let id = TestStreamId::new_chat();
    let log = FlakyLog::new(h.log.db.clone());
    let config = StreamConfig {
        log: LogConfig {
            max_retries: 2,
            ..StreamConfig::test_default().log
        },
        ..StreamConfig::test_default()
    };
    let worker = h.spawn_over_flaky_log_with(&id, log.clone(), config);
    h.subscribe(&worker).await;

    log.fail_append.store(usize::MAX, Ordering::Relaxed);
    h.send(&id, "a").await;
    worker.wake();

    wait_finished(&worker).await;
    assert_eq!(h.egress.failed(), [id]);
    assert_eq!(
        usize::MAX - log.fail_append.load(Ordering::Relaxed),
        3,
        "the first append and every retry"
    );
}
