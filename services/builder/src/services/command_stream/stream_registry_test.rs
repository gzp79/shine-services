//! The stream registry on a real Postgres and Redis, with a recording sink instead of the hub. Two kinds
//! of stream are registered: the chat room and a counter. They skip without `SHINE_TEST_PG_CNS` and
//! `SHINE_TEST_REDIS_CNS`.

use crate::{
    repositories::{
        command_inbox::{
            redis::RedisCommandInboxDb,
            tests::test_utils::{create_test_inbox, TestInbox},
            CommandInboxDb,
        },
        command_log::{
            pg::PgCommandLogDb,
            tests::test_utils::{all_commands, create_test_log, stream_id, TestLog},
            CommandLogDb, StreamId,
        },
    },
    services::command_stream::{
        stream_test_utils::{kinds, quiet, versions_of, CounterCommand, CounterState, TestEgress, WAIT},
        CommandStreamConfig, CommandStreamService, RegistryError, StreamRegistry, WorkerConfig,
    },
};
use shine_game::replica::{encode_command, ChatCommand, ChatState, ReplayState};
use shine_test::test;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

struct Harness {
    log: TestLog,
    inbox: TestInbox,
    egress: TestEgress,
    registry: StreamRegistry<PgCommandLogDb>,
}

async fn harness(worker: WorkerConfig) -> Option<Harness> {
    let log = create_test_log().await?;
    let inbox = create_test_inbox(60).await?;
    let egress = TestEgress::default();
    let service = || CommandStreamService::new(log.db.clone(), inbox.db.clone(), CommandStreamConfig::default());
    let registry = StreamRegistry::builder(log.db.clone())
        .register::<ChatState, _, _>(service(), egress.clone(), worker.clone())
        .register::<CounterState, _, _>(service(), egress.clone(), worker)
        .build()
        .unwrap();
    Some(Harness { log, inbox, egress, registry })
}

impl Harness {
    async fn push_chat(&self, stream: &StreamId, text: &str) {
        let data = encode_command::<ChatState>(&ChatCommand::Add { text: text.to_string() }).unwrap();
        self.inbox
            .db
            .push(stream, Uuid::new_v4(), Uuid::new_v4(), &data)
            .await
            .unwrap();
    }

    async fn push_counter(&self, stream: &StreamId, amount: i64) {
        let data = encode_command::<CounterState>(&CounterCommand::Add { amount }).unwrap();
        self.inbox
            .db
            .push(stream, Uuid::new_v4(), Uuid::new_v4(), &data)
            .await
            .unwrap();
    }

    /// Opens and subscribes a new connection, and waits for its sync.
    async fn join(&self, stream: &StreamId, kind: &str) -> Uuid {
        let connection = Uuid::new_v4();
        self.join_as(connection, stream, kind).await;
        connection
    }

    async fn join_as(&self, connection: Uuid, stream: &StreamId, kind: &str) {
        let kind = self.registry.open(stream, kind).await.unwrap();
        self.registry.subscribe(connection, stream, &kind).unwrap();
        self.egress
            .wait(connection, |m| m.iter().any(|message| message.stream_id() == stream))
            .await;
    }
}

async fn wait_for_workers(registry: &StreamRegistry<PgCommandLogDb>, count: usize) {
    let result = timeout(WAIT, async {
        while registry.worker_count() != count {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(result.is_ok(), "{} workers, expected {count}", registry.worker_count());
}

fn short_grace() -> WorkerConfig {
    WorkerConfig {
        unload_grace: Duration::from_millis(200),
        ..quiet()
    }
}

#[test]
async fn a_kind_registered_twice_fails_the_build() {
    let Some(log) = create_test_log().await else { return };
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let service = || CommandStreamService::new(log.db.clone(), inbox.db.clone(), CommandStreamConfig::default());

    let result = StreamRegistry::builder(log.db.clone())
        .register::<ChatState, _, _>(service(), TestEgress::default(), quiet())
        .register::<ChatState, _, _>(service(), TestEgress::default(), quiet())
        .build();

    assert!(matches!(result, Err(RegistryError::DuplicateKind(kind)) if kind == ChatState::KIND));
}

// ---------------------------------------------------------------------------------------------
// Opening
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_new_stream_opens_with_a_registered_kind_and_an_unknown_kind_does_not() {
    let Some(h) = harness(quiet()).await else { return };

    assert_eq!(
        h.registry.open(&stream_id(), ChatState::KIND).await.unwrap(),
        ChatState::KIND
    );
    let unknown = h.registry.open(&stream_id(), "nothing").await;
    assert!(matches!(unknown, Err(RegistryError::UnknownKind(kind)) if kind == "nothing"));
}

#[test]
async fn an_existing_stream_opens_only_as_its_own_kind() {
    let Some(h) = harness(quiet()).await else { return };
    let id = stream_id();
    h.join(&id, ChatState::KIND).await;

    let same = h.registry.open(&id, ChatState::KIND).await;
    let other = h.registry.open(&id, CounterState::KIND).await;

    assert_eq!(same.unwrap(), ChatState::KIND);
    assert!(matches!(
        other,
        Err(RegistryError::KindMismatch { stored, requested })
            if stored == ChatState::KIND && requested == CounterState::KIND
    ));
}

#[test]
async fn commands_are_checked_by_the_kind_of_their_stream() {
    let Some(h) = harness(quiet()).await else { return };
    let chat = encode_command::<ChatState>(&ChatCommand::Add { text: "hi".into() }).unwrap();
    let counter = encode_command::<CounterState>(&CounterCommand::Add { amount: 5 }).unwrap();
    let too_big = encode_command::<CounterState>(&CounterCommand::Add { amount: 5000 }).unwrap();

    assert!(h.registry.validate(ChatState::KIND, &chat));
    assert!(h.registry.validate(CounterState::KIND, &counter));
    assert!(!h.registry.validate(CounterState::KIND, &too_big), "not well formed");
    assert!(!h.registry.validate(ChatState::KIND, &counter), "not a chat command");
    assert!(!h.registry.validate("nothing", &chat), "unknown kind");
}

// ---------------------------------------------------------------------------------------------
// Workers
// ---------------------------------------------------------------------------------------------

#[test]
async fn subscribers_of_a_stream_share_one_worker_and_streams_have_their_own() {
    let Some(h) = harness(quiet()).await else { return };
    let (a, b) = (stream_id(), stream_id());

    let first = h.join(&a, ChatState::KIND).await;
    let second = h.join(&a, ChatState::KIND).await;
    assert_eq!(h.registry.worker_count(), 1);
    h.join(&b, ChatState::KIND).await;
    assert_eq!(h.registry.worker_count(), 2);

    // One worker serves both: a single wake delivers to both.
    h.push_chat(&a, "to both").await;
    h.registry.wake(&a);

    h.egress.wait(first, |m| versions_of(m, &a) == [1]).await;
    h.egress.wait(second, |m| versions_of(m, &a) == [1]).await;
}

#[test]
async fn subscribing_to_an_unregistered_kind_fails() {
    let Some(h) = harness(quiet()).await else { return };

    let result = h.registry.subscribe(Uuid::new_v4(), &stream_id(), "nothing");

    assert!(matches!(result, Err(RegistryError::UnknownKind(_))));
    assert_eq!(h.registry.worker_count(), 0);
}

#[test]
async fn a_wake_reaches_the_worker_of_its_stream() {
    let Some(h) = harness(quiet()).await else { return };
    let (a, b) = (stream_id(), stream_id());
    let in_a = h.join(&a, ChatState::KIND).await;
    let in_b = h.join(&b, ChatState::KIND).await;
    h.push_chat(&a, "for a").await;
    h.push_chat(&b, "for b").await;

    h.registry.wake(&a);

    h.egress.wait(in_a, |m| versions_of(m, &a) == [1]).await;
    sleep(Duration::from_millis(150)).await;
    assert_eq!(
        versions_of(&h.egress.messages(in_b), &b),
        Vec::<i64>::new(),
        "b was not woken"
    );
}

#[test]
async fn the_inbox_wakes_the_workers_without_anyone_calling_wake() {
    let Some(h) = harness(quiet()).await else { return };
    h.registry.listen(&h.inbox.db).await.unwrap();
    let id = stream_id();
    let connection = h.join(&id, ChatState::KIND).await;

    h.push_chat(&id, "a").await;
    h.push_chat(&id, "b").await;

    h.egress.wait(connection, |m| versions_of(m, &id) == [1, 2]).await;
    h.registry.unlisten(&h.inbox.db).await.unwrap();
}

#[test]
async fn streams_of_different_kinds_work_side_by_side_for_one_connection() {
    let Some(h) = harness(quiet()).await else { return };
    h.registry.listen(&h.inbox.db).await.unwrap();
    let (chat, counter) = (stream_id(), stream_id());
    let connection = h.join(&chat, ChatState::KIND).await;
    h.join_as(connection, &counter, CounterState::KIND).await;

    h.push_chat(&chat, "hello").await;
    h.push_counter(&counter, 5).await;
    h.push_counter(&counter, 7).await;

    h.egress
        .wait(connection, |m| {
            versions_of(m, &chat) == [1] && versions_of(m, &counter) == [1, 2]
        })
        .await;
    assert_eq!(all_commands(&h.log.db, &counter).await.len(), 2);
    assert_eq!(h.log.db.head(&chat).await.unwrap().unwrap().kind, ChatState::KIND);
    assert_eq!(h.log.db.head(&counter).await.unwrap().unwrap().kind, CounterState::KIND);
    h.registry.unlisten(&h.inbox.db).await.unwrap();
}

// ---------------------------------------------------------------------------------------------
// Leaving
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_disconnect_releases_every_stream_of_the_connection() {
    let Some(h) = harness(short_grace()).await else { return };
    let (chat, counter) = (stream_id(), stream_id());
    let connection = h.join(&chat, ChatState::KIND).await;
    h.join_as(connection, &counter, CounterState::KIND).await;
    assert_eq!(h.registry.worker_count(), 2);

    h.registry.disconnect(connection);

    wait_for_workers(&h.registry, 0).await;
}

#[test]
async fn an_unsubscribe_releases_only_that_stream_and_a_stream_can_be_joined_again_after_its_worker_ended() {
    let Some(h) = harness(short_grace()).await else { return };
    let (a, b) = (stream_id(), stream_id());
    let connection = h.join(&a, ChatState::KIND).await;
    h.join_as(connection, &b, ChatState::KIND).await;

    h.registry.unsubscribe(connection, &a);
    h.registry.unsubscribe(connection, &a);

    wait_for_workers(&h.registry, 1).await;
    // A is gone; joining it again starts a new worker.
    let again = Uuid::new_v4();
    h.join_as(again, &a, ChatState::KIND).await;
    assert_eq!(kinds(&h.egress.messages(again)), vec!["sync:0"]);
    assert_eq!(h.registry.worker_count(), 2);
}

#[test]
async fn a_shutdown_persists_what_is_waiting_and_ends_every_worker() {
    let Some(h) = harness(quiet()).await else { return };
    let (a, b) = (stream_id(), stream_id());
    h.join(&a, ChatState::KIND).await;
    h.join(&b, CounterState::KIND).await;
    h.push_chat(&a, "waiting").await;
    h.push_counter(&b, 3).await;

    h.registry.shutdown(WAIT).await;

    assert_eq!(h.registry.worker_count(), 0);
    assert_eq!(all_commands(&h.log.db, &a).await.len(), 1);
    assert_eq!(all_commands(&h.log.db, &b).await.len(), 1);
}
