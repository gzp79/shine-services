//! The stream registry on a real Postgres and Redis, with a recording sink instead of the hub. Two kinds
//! of stream are registered: the chat room and the pixel map.

use crate::{
    models::{messages::CommandFailure, CommandStreamError},
    repositories::{
        command_inbox::{redis::RedisCommandInboxDb, CommandInboxDb, InboxId},
        command_log::{pg::PgCommandLogDb, CommandLogDb},
    },
    services::command_stream::{
        stream_persistence::PersistenceWork, PersistResult, StreamConfig, StreamLogService, StreamPersistence,
        StreamRegistry, MAX_COMMAND_BYTES,
    },
    testing::{
        command_stream::{ChatLog, TestEgress, TestInbox, TestLog, TestStreamId},
        TestMessage,
    },
};
use shine_game::replica::{
    encode_command, ChatCommand, ChatState, PixelMapCommand, PixelMapState, ReplayState, StreamId,
};
use shine_test::test;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

struct Harness {
    log: TestLog,
    inbox: TestInbox,
    egress: TestEgress,
    registry: StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>,
}

async fn harness(worker: StreamConfig) -> Harness {
    let log = TestLog::from_env().await;
    let inbox = TestInbox::from_env().await;
    let egress = TestEgress::default();
    let service = || StreamLogService::new(inbox.db.clone(), log.db.clone(), StreamConfig::default());
    let registry = StreamRegistry::builder(inbox.db.clone(), log.db.clone())
        .register::<ChatState, _>(service(), egress.clone(), worker.clone())
        .register::<PixelMapState, _>(service(), egress.clone(), worker)
        .build()
        .unwrap();
    Harness { log, inbox, egress, registry }
}

impl Harness {
    async fn push_chat(&self, stream: &StreamId, text: &str) {
        let data = TestMessage::new_chat_command(text);
        self.inbox
            .db
            .push(stream, Uuid::new_v4(), Uuid::new_v4(), &data)
            .await
            .unwrap();
    }

    async fn push_paint(&self, stream: &StreamId, x: u8, y: u8, color: u8) {
        let data = TestMessage::new_paint_command(x, y, color);
        self.inbox
            .db
            .push(stream, Uuid::new_v4(), Uuid::new_v4(), &data)
            .await
            .unwrap();
    }

    /// Opens and subscribes a new connection, and waits for its sync.
    async fn join(&self, stream: &StreamId) -> Uuid {
        let connection = Uuid::new_v4();
        self.join_as(connection, stream).await;
        connection
    }

    async fn join_as(&self, connection: Uuid, stream: &StreamId) {
        self.registry.open(connection, stream).unwrap();
        self.registry.subscribe(connection, stream).unwrap();
        self.egress
            .wait(connection, |m| m.iter().any(|message| message.stream_id() == stream))
            .await;
    }
}

async fn wait_for_workers(registry: &StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>, count: usize) {
    let result = timeout(WAIT_TIMEOUT, async {
        while registry.worker_count() != count {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(result.is_ok(), "{} workers, expected {count}", registry.worker_count());
}

fn short_grace() -> StreamConfig {
    StreamConfig {
        worker_unload_grace: Duration::from_millis(200),
        ..StreamConfig::quiet()
    }
}

#[test]
async fn a_kind_registered_twice_fails_the_build() {
    let log = TestLog::from_env().await;
    let inbox = TestInbox::from_env().await;
    let service = || StreamLogService::new(inbox.db.clone(), log.db.clone(), StreamConfig::default());

    let result = StreamRegistry::builder(inbox.db.clone(), log.db.clone())
        .register::<ChatState, _>(service(), TestEgress::default(), StreamConfig::quiet())
        .register::<ChatState, _>(service(), TestEgress::default(), StreamConfig::quiet())
        .build();

    assert!(matches!(result, Err(CommandStreamError::DuplicateKind(kind)) if kind == ChatState::KIND));
}

// ---------------------------------------------------------------------------------------------
// Opening
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_new_stream_opens_if_its_kind_is_registered() {
    let h = harness(StreamConfig::quiet()).await;

    assert!(h.registry.open(Uuid::new_v4(), &TestStreamId::new_chat()).is_ok());
}

#[test]
async fn a_push_needs_the_stream_open_for_the_connection() {
    let h = harness(StreamConfig::quiet()).await;
    let (id, other) = (TestStreamId::new_chat(), TestStreamId::new_chat());
    let connection = Uuid::new_v4();
    let data = TestMessage::new_chat_command("hi");
    let push = |stream: &StreamId| {
        let (registry, stream, data) = (h.registry.clone(), stream.clone(), data.clone());
        async move {
            registry
                .push(connection, &stream, Uuid::nil(), Uuid::new_v4(), &data)
                .await
        }
    };

    assert_eq!(push(&id).await, Err(CommandFailure::NotSubscribed), "not opened");
    h.registry.open(connection, &id).unwrap();
    h.registry.open(Uuid::new_v4(), &other).unwrap();
    assert_eq!(push(&id).await, Ok(()));
    assert_eq!(
        push(&other).await,
        Err(CommandFailure::NotSubscribed),
        "opened by another"
    );
    h.registry.close(connection, &id);
    assert_eq!(push(&id).await, Err(CommandFailure::NotSubscribed), "closed");
    h.registry.open(connection, &id).unwrap();
    h.registry.disconnect(connection);
    assert_eq!(push(&id).await, Err(CommandFailure::NotSubscribed), "disconnected");
}

#[test]
async fn commands_are_checked_by_the_kind_of_their_stream() {
    let h = harness(StreamConfig::quiet()).await;
    let chat = TestMessage::new_chat_command("hi");
    let paint = TestMessage::new_paint_command(1, 2, 3);
    let off_the_map = TestMessage::new_paint_command(99, 2, 3);

    assert!(h.registry.validate(ChatState::KIND, &chat));
    assert!(h.registry.validate(PixelMapState::KIND, &paint));
    assert!(
        !h.registry.validate(PixelMapState::KIND, &off_the_map),
        "not well formed"
    );
    assert!(!h.registry.validate(ChatState::KIND, &paint), "not a chat command");
}

#[test]
async fn a_valid_command_is_queued_in_the_inbox_of_its_stream() {
    let h = harness(StreamConfig::quiet()).await;
    let id = TestStreamId::new_chat();
    let (connection, command_id, user_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let data = TestMessage::new_chat_command("hi");
    h.registry.open(connection, &id).unwrap();

    h.registry
        .push(connection, &id, user_id, command_id, &data)
        .await
        .unwrap();

    let queued = h.inbox.db.poll(&id, &InboxId::START, 10).await.unwrap().entries;
    assert_eq!(queued.len(), 1);
    assert_eq!((queued[0].command_id, queued[0].user_id), (command_id, user_id));
}

#[test]
async fn a_command_that_is_not_valid_for_the_kind_is_refused_and_not_queued() {
    let h = harness(StreamConfig::quiet()).await;
    let id = TestStreamId::new_chat();
    let connection = Uuid::new_v4();
    let paint = TestMessage::new_paint_command(1, 1, 1);
    let off_the_map = TestMessage::new_paint_command(90, 1, 1);
    let too_long = TestMessage::new_chat_command(&"a".repeat(MAX_COMMAND_BYTES + 1));
    h.registry.open(connection, &id).unwrap();
    let push = |command: Vec<u8>| {
        let (registry, id) = (h.registry.clone(), id.clone());
        async move {
            registry
                .push(connection, &id, Uuid::nil(), Uuid::new_v4(), &command)
                .await
        }
    };

    let results = [
        push(paint).await,
        push(off_the_map).await,
        push(b"{not json".to_vec()).await,
        push(too_long).await,
    ];

    assert!(
        results.iter().all(|r| *r == Err(CommandFailure::Invalid)),
        "{results:?}"
    );
    assert!(h
        .inbox
        .db
        .poll(&id, &InboxId::START, 10)
        .await
        .unwrap()
        .entries
        .is_empty());
}

#[test]
async fn a_push_into_an_inbox_that_stays_full_fails_as_busy_and_is_not_queued() {
    let h = harness(StreamConfig {
        inbox_backpressure_low: 0,
        inbox_backpressure_high: 1,
        inbox_backpressure_poll_period: Duration::from_millis(5),
        inbox_backpressure_timeout: Duration::from_millis(100),
        ..StreamConfig::quiet()
    })
    .await;
    // No worker is subscribed, so nothing drains the inbox.
    let id = TestStreamId::new_chat();
    let connection = Uuid::new_v4();
    let data = TestMessage::new_chat_command("hi");
    h.registry.open(connection, &id).unwrap();

    h.registry
        .push(connection, &id, Uuid::nil(), Uuid::new_v4(), &data)
        .await
        .unwrap();
    // The limit is polled, not reserved: the next push is only throttled once the length is seen.
    sleep(Duration::from_millis(100)).await;
    let result = h
        .registry
        .push(connection, &id, Uuid::nil(), Uuid::new_v4(), &data)
        .await;

    assert_eq!(result, Err(CommandFailure::Busy));
    assert_eq!(h.inbox.db.len(&id).await.unwrap(), 1);
}

// ---------------------------------------------------------------------------------------------
// Workers
// ---------------------------------------------------------------------------------------------

#[test]
async fn subscribers_of_a_stream_share_one_worker_and_streams_have_their_own() {
    let h = harness(StreamConfig::quiet()).await;
    let (a, b) = (TestStreamId::new_chat(), TestStreamId::new_pixel_map());

    let first = h.join(&a).await;
    let second = h.join(&a).await;
    assert_eq!(h.registry.worker_count(), 1);
    h.join(&b).await;
    assert_eq!(h.registry.worker_count(), 2);

    // One worker serves both: a single wake delivers to both.
    h.push_chat(&a, "to both").await;
    h.registry.wake(&a);

    h.egress.wait(first, |m| TestEgress::versions(m, &a) == [1]).await;
    h.egress.wait(second, |m| TestEgress::versions(m, &a) == [1]).await;
}

#[test]
async fn a_wake_reaches_the_worker_of_its_stream() {
    let h = harness(StreamConfig::quiet()).await;
    let (a, b) = (TestStreamId::new_chat(), TestStreamId::new_pixel_map());
    let in_a = h.join(&a).await;
    let in_b = h.join(&b).await;
    h.push_chat(&a, "for a").await;
    h.push_chat(&b, "for b").await;

    h.registry.wake(&a);

    h.egress.wait(in_a, |m| TestEgress::versions(m, &a) == [1]).await;
    sleep(Duration::from_millis(150)).await;
    assert_eq!(
        TestEgress::versions(&h.egress.messages(in_b), &b),
        Vec::<i64>::new(),
        "b was not woken"
    );
}

#[test]
async fn the_inbox_wakes_the_workers_without_anyone_calling_wake() {
    let h = harness(StreamConfig::quiet()).await;
    h.registry.listen().await.unwrap();
    let id = TestStreamId::new_chat();
    let connection = h.join(&id).await;

    h.push_chat(&id, "a").await;
    h.push_chat(&id, "b").await;

    h.egress
        .wait(connection, |m| TestEgress::versions(m, &id) == [1, 2])
        .await;
    h.registry.unlisten().await.unwrap();
}

#[test]
async fn streams_of_different_kinds_work_side_by_side_for_one_connection() {
    let h = harness(StreamConfig::quiet()).await;
    h.registry.listen().await.unwrap();
    let (chat, map) = (TestStreamId::new_chat(), TestStreamId::new_pixel_map());
    let connection = h.join(&chat).await;
    h.join_as(connection, &map).await;

    h.push_chat(&chat, "hello").await;
    h.push_paint(&map, 1, 1, 5).await;
    h.push_paint(&map, 2, 2, 7).await;

    h.egress
        .wait(connection, |m| {
            TestEgress::versions(m, &chat) == [1] && TestEgress::versions(m, &map) == [1, 2]
        })
        .await;
    // The subscribers are told before the log is written, so durability is awaited separately.
    h.log.wait_commands(&map, 2).await;
    h.registry.unlisten().await.unwrap();
}

#[test]
async fn the_entries_of_the_stored_commands_are_cleared_from_the_inbox() {
    let h = harness(StreamConfig::quiet()).await;
    h.registry.listen().await.unwrap();
    let id = TestStreamId::new_chat();
    let connection = h.join(&id).await;

    h.push_chat(&id, "one").await;
    h.push_chat(&id, "two").await;

    h.egress
        .wait(connection, |m| TestEgress::versions(m, &id) == [1, 2])
        .await;
    h.log.wait_commands(&id, 2).await;
    h.inbox.wait_empty(&id).await;
    h.registry.unlisten().await.unwrap();
}

#[test(skip = "the quiet-stream reconcile of the stream worker is removed for now")]
async fn a_quiet_worker_reconciles_a_durable_head_advanced_by_a_peer() {
    let h = harness(StreamConfig {
        inbox_recovery_poll_interval: Duration::from_millis(50),
        ..StreamConfig::quiet()
    })
    .await;
    let id = TestStreamId::new_chat();
    let connection = h.join(&id).await;

    let mut peer = StreamPersistence::<ChatState, _, _>::new(
        id.clone(),
        StreamLogService::new(h.inbox.db.clone(), h.log.db.clone(), StreamConfig::default()),
    );
    peer.persist(PersistenceWork::Reconcile { version: 0, chain_hash: 0 })
        .await;
    let room = ChatLog::new("stored by peer", 1);
    assert!(matches!(
        peer.persist(PersistenceWork::Persist {
            baseline_version: 0,
            baseline_chain_hash: 0,
            commands: room.commands.into(),
        })
        .await,
        PersistResult::Advanced { .. }
    ));

    h.egress
        .wait(connection, |messages| {
            TestEgress::kinds(messages) == ["sync:0", "reset", "sync:1"]
        })
        .await;
}

// ---------------------------------------------------------------------------------------------
// Leaving
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_disconnect_releases_every_stream_of_the_connection() {
    let h = harness(short_grace()).await;
    let (chat, map) = (TestStreamId::new_chat(), TestStreamId::new_chat());
    let connection = h.join(&chat).await;
    h.join_as(connection, &map).await;
    assert_eq!(h.registry.worker_count(), 2);

    h.registry.disconnect(connection);

    wait_for_workers(&h.registry, 0).await;
}

#[test]
async fn an_unsubscribe_releases_only_that_stream_and_a_stream_can_be_joined_again_after_its_worker_ended() {
    let h = harness(short_grace()).await;
    let (a, b) = (TestStreamId::new_chat(), TestStreamId::new_chat());
    let connection = h.join(&a).await;
    h.join_as(connection, &b).await;

    h.registry.close(connection, &a);
    h.registry.close(connection, &a);

    wait_for_workers(&h.registry, 1).await;
    // A is gone; joining it again starts a new worker.
    let again = Uuid::new_v4();
    h.join_as(again, &a).await;
    assert_eq!(TestEgress::kinds(&h.egress.messages(again)), vec!["sync:0"]);
    assert_eq!(h.registry.worker_count(), 2);
}

#[test]
async fn ended_workers_are_not_running_and_a_purge_forgets_them() {
    let h = harness(short_grace()).await;
    let id = TestStreamId::new_chat();
    let connection = h.join(&id).await;

    h.registry.close(connection, &id);

    wait_for_workers(&h.registry, 0).await;
    assert_eq!(h.registry.workers_count(), 1, "an ended worker is held until a purge");
    h.registry.purge();
    assert_eq!(h.registry.workers_count(), 0);
}

#[test]
async fn a_subscribe_of_a_stream_closed_since_it_was_opened_starts_no_worker() {
    let h = harness(StreamConfig::quiet()).await;
    let (closed, gone) = (TestStreamId::new_chat(), TestStreamId::new_chat());
    let connection = Uuid::new_v4();

    h.registry.open(connection, &closed).unwrap();
    h.registry.close(connection, &closed);
    h.registry.subscribe(connection, &closed).unwrap();
    h.registry.open(connection, &gone).unwrap();
    h.registry.disconnect(connection);
    h.registry.subscribe(connection, &gone).unwrap();

    assert_eq!(h.registry.workers_count(), 0);
}

#[test]
async fn a_purge_keeps_the_open_streams_and_forgets_the_rest() {
    let h = harness(StreamConfig::quiet()).await;
    let (kept, forgotten) = (TestStreamId::new_chat(), TestStreamId::new_chat());
    let (connection, other) = (Uuid::new_v4(), Uuid::new_v4());
    h.registry.open(connection, &kept).unwrap();
    h.registry.open(other, &forgotten).unwrap();
    h.registry.disconnect(other);

    h.registry.purge();

    let state = h.registry.inner.state.lock().unwrap();
    assert!(state.streams.contains_key(&kept), "open, even without a worker yet");
    assert!(!state.streams.contains_key(&forgotten));
}

#[test]
async fn a_shutdown_persists_what_is_waiting_and_ends_every_worker() {
    let h = harness(StreamConfig::quiet()).await;
    let (a, b) = (TestStreamId::new_chat(), TestStreamId::new_pixel_map());
    h.join(&a).await;
    h.join(&b).await;
    h.push_chat(&a, "waiting").await;
    h.push_paint(&b, 3, 3, 3).await;

    h.registry.shutdown(WAIT_TIMEOUT).await;

    assert_eq!(h.registry.worker_count(), 0);
    assert_eq!(h.log.all_commands(&a).await.len(), 1);
    assert_eq!(h.log.all_commands(&b).await.len(), 1);
}
