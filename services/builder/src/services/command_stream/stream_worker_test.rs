//! The stream worker on a real Postgres and Redis, with a recording sink instead of the hub. The wake that
//! the inbox listener will give a worker is simulated by calling `wake`.

use crate::{
    models::{messages::StreamMessage, CommandStreamError},
    repositories::{
        command_inbox::{redis::RedisCommandInboxDb, CommandInboxDb, InboxBatch, InboxId, InboxWake},
        command_log::{pg::PgCommandLogDb, CommandLogDb},
    },
    services::command_stream::{StreamConfig, StreamLogService, StreamWorker},
    testing::command_stream::{
        create_test_inbox, create_test_log, fetch_all_commands, message_kinds, message_versions, persist_chat_logs,
        replay_chat, stream_id, ChatLog, TestEgress, TestInbox, TestLog, WAIT_NOTIFICATION,
    },
};
use shine_game::replica::{encode_command, ChatCommand, ChatState, ReplayState, StreamId};
use shine_test::test;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

pub async fn wait_finished(worker: &StreamWorker) {
    let result = timeout(WAIT_NOTIFICATION, worker.wait_finished()).await;
    assert!(result.is_ok(), "the worker did not end");
}

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
    fn spawn(&self, stream: &StreamId, every: i64, mut config: StreamConfig) -> Arc<StreamWorker> {
        config.snapshot_every = every;
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
        let data = encode_command::<ChatState>(&ChatCommand::Add { text: text.to_string() }).unwrap();
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
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(&id, 100, StreamConfig::quiet());

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
        .wait(connection, |m| message_versions(m, &id) == [1, 2, 3])
        .await;
    for text in ["d", "e"] {
        h.send(&id, text).await;
    }
    worker.wake();
    h.egress
        .wait(connection, |m| message_versions(m, &id) == [1, 2, 3, 4, 5])
        .await;

    worker.wake();
    sleep(Duration::from_millis(150)).await;
    assert_eq!(
        message_versions(&h.egress.messages(connection), &id),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
async fn a_backlog_larger_than_one_poll_is_worked_off_without_waiting_for_another_wake() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn_with(
        &id,
        StreamConfig {
            poll_limit: 5,
            ..StreamConfig::quiet()
        },
    );
    let connection = h.subscribe(&worker).await;
    for i in 0..12 {
        h.send(&id, &format!("m{i}")).await;
    }

    worker.wake();

    h.egress
        .wait(connection, |m| message_versions(m, &id) == (1..=12).collect::<Vec<_>>())
        .await;
}

#[test]
async fn a_late_subscriber_gets_the_snapshot_and_the_tail_and_then_only_newer_commands() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(&id, 4, StreamConfig::quiet());
    let early = h.subscribe(&worker).await;
    for i in 0..10 {
        h.send(&id, &format!("m{i}")).await;
    }
    worker.wake();
    h.egress.wait(early, |m| message_versions(m, &id).len() == 10).await;
    h.egress
        .wait(early, |m| {
            m.iter()
                .any(|message| matches!(message.as_ref(), StreamMessage::Checkpoint { version: 8, .. }))
        })
        .await;

    let late = h.subscribe(&worker).await;

    let commands = fetch_all_commands(&h.log.db, &id).await;
    let at_8 = replay_chat(None, &commands[..8]);
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
    h.egress.wait(late, |m| message_versions(m, &id) == [9, 10, 11]).await;
}

#[test]
async fn a_checkpoint_is_sent_to_the_subscribers_with_its_state_hash() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(&id, 4, StreamConfig::quiet());
    let connection = h.subscribe(&worker).await;
    for i in 0..9 {
        h.send(&id, &format!("m{i}")).await;
    }

    worker.wake();
    h.egress.wait(connection, |m| message_versions(m, &id).len() == 9).await;

    let expected = replay_chat(None, &fetch_all_commands(&h.log.db, &id).await[..8]).hash();
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
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(
        &id,
        100,
        StreamConfig {
            backstop: Duration::from_millis(50),
            ..StreamConfig::quiet()
        },
    );
    let connection = h.subscribe(&worker).await;

    h.send(&id, "a").await;

    h.egress.wait(connection, |m| message_versions(m, &id) == [1]).await;
}

#[test]
async fn a_subscriber_that_cannot_be_sent_to_is_forgotten() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(&id, 100, StreamConfig::quiet());
    let steady = h.subscribe(&worker).await;
    let flaky = h.subscribe(&worker).await;
    h.send(&id, "a").await;
    worker.wake();
    h.egress.wait(flaky, |m| message_versions(m, &id) == [1]).await;

    h.egress.kill(flaky);
    h.send(&id, "b").await;
    worker.wake();
    h.egress.wait(steady, |m| message_versions(m, &id) == [1, 2]).await;
    h.egress.revive(flaky);
    h.send(&id, "c").await;
    worker.wake();

    h.egress.wait(steady, |m| message_versions(m, &id) == [1, 2, 3]).await;
    assert_eq!(
        message_versions(&h.egress.messages(flaky), &id),
        vec![1],
        "it was dropped"
    );
}

// ---------------------------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------------------------

fn short_grace_config() -> StreamConfig {
    StreamConfig {
        unload_grace: Duration::from_millis(300),
        ..StreamConfig::quiet()
    }
}

#[test]
async fn a_subscriber_coming_back_within_the_grace_period_finds_the_stream_loaded() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(&id, 100, short_grace_config());
    let connection = h.subscribe(&worker).await;

    worker.unsubscribe(connection);
    assert!(worker.subscribe(connection));
    h.egress
        .wait(connection, |m| message_kinds(m) == ["sync:0", "sync:0"])
        .await;
    sleep(Duration::from_millis(400)).await;

    assert!(!worker.is_finished(), "the new subscriber keeps the worker");
    h.send(&id, "a").await;
    worker.wake();
    h.egress.wait(connection, |m| message_versions(m, &id) == [1]).await;
}

#[test]
async fn the_worker_ends_when_the_grace_period_after_the_last_subscriber_is_over() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
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
async fn commands_waiting_in_the_inbox_are_persisted_before_the_worker_ends() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let worker = h.spawn(&id, 100, short_grace_config());
    let connection = h.subscribe(&worker).await;
    h.send(&id, "last words").await;

    worker.unsubscribe(connection);

    wait_finished(&worker).await;
    assert_eq!(fetch_all_commands(&h.log.db, &id).await.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Drift and failures
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_log_contradicting_the_worker_resets_the_subscribers_and_syncs_them_again() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    h.log.db.ensure_stream(&id).await.unwrap();
    persist_chat_logs(&h.log.db, &id, &ChatLog::new("seed", 3), 1, 3)
        .await
        .unwrap();
    let worker = h.spawn(&id, 100, StreamConfig::quiet());
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
    persist_chat_logs(&h.log.db, &id, &ChatLog::new("other", 4), 1, 4)
        .await
        .unwrap();
    h.send(&id, "mine").await;

    worker.wake();
    h.egress
        .wait(connection, |m| {
            message_kinds(m) == ["sync:3", "commands:1", "reset", "sync:4", "commands:1"]
        })
        .await;
}

#[test]
async fn a_stream_that_cannot_be_loaded_tells_the_waiting_subscribers_and_ends() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let mut room = ChatLog::new("seed", 3);
    room.commands[1].data = "{not a command".to_string();
    h.log.db.ensure_stream(&id).await.unwrap();
    persist_chat_logs(&h.log.db, &id, &room, 1, 3).await.unwrap();
    let worker = h.spawn(&id, 100, StreamConfig::quiet());
    let connection = Uuid::new_v4();
    assert!(worker.subscribe(connection));

    h.egress.wait(connection, |m| message_kinds(m) == ["unsynced"]).await;

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
    ) -> Result<InboxId, CommandStreamError> {
        self.inner.push(stream_id, command_id, user_id, data).await
    }

    async fn poll(
        &self,
        stream_id: &StreamId,
        after: &InboxId,
        limit: usize,
    ) -> Result<InboxBatch, CommandStreamError> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(CommandStreamError::Corrupt("injected failure".to_string()));
        }
        self.inner.poll(stream_id, after, limit).await
    }

    async fn trim(&self, stream_id: &StreamId, up_to: &InboxId) -> Result<usize, CommandStreamError> {
        self.inner.trim(stream_id, up_to).await
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
async fn a_stream_that_keeps_failing_is_given_up_and_its_subscribers_are_told() {
    let Some(h) = harness().await else { return };
    let id = stream_id(ChatState::KIND);
    let fail = Arc::new(AtomicBool::new(false));
    let flaky = FlakyInbox {
        inner: h.inbox.db.clone(),
        fail: fail.clone(),
    };
    let config = StreamConfig {
        backstop: Duration::from_millis(30),
        max_failures: 2,
        ..StreamConfig::quiet()
    };
    let service = StreamLogService::new(flaky.clone(), h.log.db.clone(), config.clone());
    let worker = StreamWorker::start::<ChatState, _, _, _>(service, flaky, h.egress.clone(), id.clone(), config);
    let connection = h.subscribe(&worker).await;

    fail.store(true, Ordering::Relaxed);

    h.egress
        .wait(connection, |m| message_kinds(m) == ["sync:0", "unsynced"])
        .await;
    wait_finished(&worker).await;
    assert!(!worker.subscribe(Uuid::new_v4()));
}
