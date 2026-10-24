//! The command stream service on a real Postgres and Redis, with the chat room as the replayed state.
//! Several instances are simulated by several `StreamState`s of one stream sharing one service. They
//! skip without `SHINE_TEST_PG_CNS` and `SHINE_TEST_REDIS_CNS`.

use crate::{
    repositories::{
        command_inbox::{
            redis::RedisCommandInboxDb,
            tests::test_utils::{create_test_inbox, next_wake_of, TestInbox},
            CommandInboxDb, InboxId,
        },
        command_log::{
            pg::PgCommandLogDb,
            tests::test_utils::{
                all_commands, append_range, create_test_log, stream_id, stream_with_snapshots, ChatLog, TestLog, KIND,
            },
            CommandLogDb, StoredCommand, StreamId,
        },
    },
    services::command_stream::{CommandStreamConfig, CommandStreamService, StreamState},
};
use shine_game::replica::{decode_command, encode_command, ChatCommand, ChatState};
use shine_test::test;
use uuid::Uuid;

type Service = CommandStreamService<PgCommandLogDb, RedisCommandInboxDb>;

struct Fixture {
    log: TestLog,
    inbox: TestInbox,
    service: Service,
}

async fn fixture() -> Option<Fixture> {
    let log = create_test_log().await?;
    let inbox = create_test_inbox(60).await?;
    let service = CommandStreamService::new(log.db.clone(), inbox.db.clone(), CommandStreamConfig::default());
    Some(Fixture { log, inbox, service })
}

impl Fixture {
    async fn load(&self, id: &StreamId) -> StreamState<ChatState> {
        self.service.load::<ChatState>(id).await.unwrap()
    }

    /// Sends a chat message the way an ingest does; returns the command id.
    async fn send(&self, id: &StreamId, text: &str) -> Uuid {
        let command_id = Uuid::new_v4();
        self.send_as(id, command_id, text).await;
        command_id
    }

    async fn send_as(&self, id: &StreamId, command_id: Uuid, text: &str) {
        let data = encode_command::<ChatState>(&ChatCommand::Add { text: text.to_string() }).unwrap();
        self.inbox.db.push(id, command_id, Uuid::new_v4(), &data).await.unwrap();
    }

    async fn stored_texts(&self, id: &StreamId) -> Vec<String> {
        texts(&all_commands(&self.log.db, id).await)
    }

    async fn assert_log_is_sound(&self, id: &StreamId) {
        let report = self.log.db.check_consistency(id).await.unwrap();
        assert_eq!(report.issues, vec![]);
    }
}

fn texts(commands: &[StoredCommand]) -> Vec<String> {
    commands
        .iter()
        .map(|c| match decode_command::<ChatState>(&c.data).unwrap() {
            ChatCommand::Add { text } => text,
            other => panic!("not a message: {other:?}"),
        })
        .collect()
}

fn versions(commands: &[StoredCommand]) -> Vec<i64> {
    commands.iter().map(|c| c.version).collect()
}

fn strs(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

// ---------------------------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_new_stream_loads_empty() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();

    let state = fx.load(&id).await;

    assert_eq!((state.confirmed_version(), state.confirmed_chain_hash()), (0, 0));
    assert!(
        fx.log.db.head(&id).await.unwrap().is_some(),
        "loading creates the stream"
    );
}

#[test]
async fn a_stream_loads_from_its_latest_snapshot_and_the_commands_after_it() {
    let Some(fx) = fixture().await else { return };
    let (id, room) = stream_with_snapshots(&fx.log, 10, &[4, 7]).await;

    let state = fx.load(&id).await;

    assert_eq!(state.confirmed_version(), 10);
    assert_eq!(state.confirmed_chain_hash(), room.chain_hash_at(10));
    assert_eq!(state.replica().state(), room.replica().state());
    assert_eq!(
        versions(state.tail()),
        vec![8, 9, 10],
        "the commands after the snapshot"
    );
}

#[test]
async fn a_stream_of_another_kind_does_not_load() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    fx.log.db.ensure_stream(&id, "other").await.unwrap();

    let result = fx.service.load::<ChatState>(&id).await;

    assert!(result.is_err(), "{KIND} over a stream of another kind");
}

// ---------------------------------------------------------------------------------------------
// Sequencing
// ---------------------------------------------------------------------------------------------

#[test]
async fn commands_get_dense_versions_in_inbox_order_and_are_stored_and_the_inbox_is_trimmed() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut state = fx.load(&id).await;
    fx.send(&id, "a").await;
    fx.send(&id, "b").await;

    let outcome = fx.service.sync(&mut state).await.unwrap();
    let again = fx.service.sync(&mut state).await.unwrap();

    assert!(!outcome.reload);
    assert_eq!(versions(&outcome.confirmed), vec![1, 2]);
    assert_eq!(texts(&outcome.confirmed), strs(&["a", "b"]));
    assert!(again.confirmed.is_empty() && !again.reload);
    let head = fx.log.db.head(&id).await.unwrap().unwrap();
    assert_eq!((head.version, head.chain_hash), (2, state.confirmed_chain_hash()));
    assert_eq!(fx.stored_texts(&id).await, strs(&["a", "b"]));
    fx.assert_log_is_sound(&id).await;
    let left = fx.inbox.db.poll(&id, &InboxId::START, 10).await.unwrap();
    assert!(left.entries.is_empty(), "the persisted commands left the inbox");
}

#[test]
async fn a_loaded_stream_continues_after_its_head() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut first = fx.load(&id).await;
    fx.send(&id, "a").await;
    fx.send(&id, "b").await;
    fx.service.sync(&mut first).await.unwrap();

    let mut second = fx.load(&id).await;
    fx.send(&id, "c").await;
    let outcome = fx.service.sync(&mut second).await.unwrap();

    assert_eq!(second.confirmed_version(), 3);
    assert_eq!(versions(&outcome.confirmed), vec![3]);
    assert_eq!(fx.stored_texts(&id).await, strs(&["a", "b", "c"]));
    fx.assert_log_is_sound(&id).await;
}

#[test]
async fn command_ids_are_the_clients_own_so_a_repeated_one_gets_a_version_each() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut state = fx.load(&id).await;
    let command_id = fx.send(&id, "same").await;
    fx.send_as(&id, command_id, "same").await;
    fx.send_as(&id, command_id, "same id, other text").await;

    let outcome = fx.service.sync(&mut state).await.unwrap();

    assert_eq!(
        texts(&outcome.confirmed),
        strs(&["same", "same", "same id, other text"])
    );
    assert_eq!(versions(&outcome.confirmed), vec![1, 2, 3]);
}

#[test]
async fn an_entry_that_cannot_be_decoded_is_skipped_without_blocking_the_rest() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut state = fx.load(&id).await;
    let (command, user) = (Uuid::new_v4().to_string(), Uuid::new_v4().to_string());
    fx.inbox
        .raw_add(
            &id,
            &[("command_id", &command), ("user_id", &user), ("data", "{not a command")],
        )
        .await;
    fx.send(&id, "fine").await;

    let outcome = fx.service.sync(&mut state).await.unwrap();

    assert_eq!(texts(&outcome.confirmed), strs(&["fine"]));
    assert_eq!(versions(&outcome.confirmed), vec![1]);
}

#[test]
async fn the_persisted_version_is_announced_to_the_other_instances() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut state = fx.load(&id).await;
    let mut wakes = fx.inbox.listen().await;
    fx.send(&id, "a").await;
    fx.send(&id, "b").await;

    fx.service.sync(&mut state).await.unwrap();

    loop {
        let wake = next_wake_of(&mut wakes, &id).await;
        if let crate::repositories::command_inbox::InboxWake::Stream { persisted: Some(version), .. } = wake {
            assert_eq!(version, 2);
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Instances that are behind
// ---------------------------------------------------------------------------------------------

#[test]
async fn an_instance_behind_adopts_what_the_others_persisted_and_sequences_its_own_command_after_it() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut ahead = fx.load(&id).await;
    let mut behind = fx.load(&id).await;
    for text in ["a", "b", "c"] {
        fx.send(&id, text).await;
    }
    fx.service.sync(&mut ahead).await.unwrap();
    // The inbox only holds this one now: alone, it would be version 1.
    fx.send(&id, "d").await;

    let outcome = fx.service.sync(&mut behind).await.unwrap();

    assert!(!outcome.reload);
    assert_eq!(texts(&outcome.confirmed), strs(&["a", "b", "c", "d"]));
    assert_eq!(versions(&outcome.confirmed), vec![1, 2, 3, 4]);
    assert_eq!(behind.confirmed_version(), 4);
    assert_eq!(fx.stored_texts(&id).await, strs(&["a", "b", "c", "d"]));
    fx.assert_log_is_sound(&id).await;
}

#[test]
async fn a_batch_overlapping_commands_of_the_others_is_resequenced_after_them() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut ahead = fx.load(&id).await;
    let mut behind = fx.load(&id).await;
    fx.send(&id, "a").await;
    fx.service.sync(&mut ahead).await.unwrap();
    fx.send(&id, "d").await;
    fx.send(&id, "e").await;

    let outcome = fx.service.sync(&mut behind).await.unwrap();

    assert_eq!(texts(&outcome.confirmed), strs(&["a", "d", "e"]));
    assert_eq!(versions(&outcome.confirmed), vec![1, 2, 3]);
    assert_eq!(fx.stored_texts(&id).await, strs(&["a", "d", "e"]));
    fx.assert_log_is_sound(&id).await;
}

#[test]
async fn a_waiting_command_that_another_instance_stored_is_not_appended_again() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut ahead = fx.load(&id).await;
    let mut behind = fx.load(&id).await;
    // Reads the inbox but persists nothing: `x` waits in `behind`.
    let reads_only = CommandStreamService::new(
        fx.log.db.clone(),
        fx.inbox.db.clone(),
        CommandStreamConfig {
            max_attempts: 0,
            ..CommandStreamConfig::default()
        },
    );
    fx.send(&id, "x").await;
    reads_only.sync(&mut behind).await.unwrap();
    assert_eq!(behind.pending_len(), 1);
    fx.send(&id, "y").await;
    fx.service.sync(&mut ahead).await.unwrap();

    let outcome = fx.service.sync(&mut behind).await.unwrap();

    assert_eq!(texts(&outcome.confirmed), strs(&["x", "y"]));
    assert_eq!(behind.pending_len(), 0);
    assert_eq!(fx.stored_texts(&id).await, strs(&["x", "y"]));
    fx.assert_log_is_sound(&id).await;
}

#[test]
async fn a_waiting_command_is_kept_when_the_stored_one_with_its_id_is_another_command() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut ahead = fx.load(&id).await;
    let mut behind = fx.load(&id).await;
    let reads_only = CommandStreamService::new(
        fx.log.db.clone(),
        fx.inbox.db.clone(),
        CommandStreamConfig {
            max_attempts: 0,
            ..CommandStreamConfig::default()
        },
    );
    let reused = Uuid::new_v4();
    fx.send_as(&id, reused, "first use").await;
    fx.service.sync(&mut ahead).await.unwrap();
    fx.send_as(&id, reused, "second use").await;
    reads_only.sync(&mut behind).await.unwrap();

    let outcome = fx.service.sync(&mut behind).await.unwrap();

    assert_eq!(texts(&outcome.confirmed), strs(&["first use", "second use"]));
    assert_eq!(fx.stored_texts(&id).await, strs(&["first use", "second use"]));
}

#[test]
async fn inbox_entries_that_are_stored_but_not_trimmed_are_not_taken_again() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut first = fx.load(&id).await;
    fx.send(&id, "x").await;
    fx.send(&id, "y").await;
    let stored = fx.service.sync(&mut first).await.unwrap().confirmed;
    // As if the trim of the instance that stored them had not happened yet.
    fx.inbox.drop_inbox(&id).await;
    for command in &stored {
        let at = InboxId::parse(command.inbox_id.as_deref().unwrap()).unwrap();
        let fields = [
            ("command_id", command.command_id.to_string()),
            ("user_id", command.user_id.to_string()),
            ("data", command.data.clone()),
        ];
        fx.inbox
            .raw_add_at(
                &id,
                &at,
                &fields.each_ref().map(|(name, value)| (*name, value.as_str())),
            )
            .await;
    }
    fx.send(&id, "z").await;
    let mut late = fx.load(&id).await;

    let outcome = fx.service.sync(&mut late).await.unwrap();

    assert_eq!(texts(&outcome.confirmed), strs(&["z"]));
    assert_eq!(versions(&outcome.confirmed), vec![3]);
    assert_eq!(fx.stored_texts(&id).await, strs(&["x", "y", "z"]));
}

#[test]
async fn an_idle_instance_adopts_what_the_others_persisted() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let mut busy = fx.load(&id).await;
    let mut idle = fx.load(&id).await;
    fx.send(&id, "a").await;
    fx.send(&id, "b").await;
    fx.service.sync(&mut busy).await.unwrap();

    let outcome = fx.service.sync(&mut idle).await.unwrap();

    assert_eq!(texts(&outcome.confirmed), strs(&["a", "b"]));
    assert_eq!(idle.confirmed_version(), 2);
    assert_eq!(idle.confirmed_chain_hash(), busy.confirmed_chain_hash());
}

#[test]
async fn instances_sequencing_the_same_inbox_at_once_agree() {
    let Some(fx) = fixture().await else { return };

    for round in 0..8 {
        let id = stream_id();
        let mut first = fx.load(&id).await;
        let mut second = fx.load(&id).await;
        let expected: Vec<String> = (0..6).map(|i| format!("m{i}")).collect();
        for text in &expected {
            fx.send(&id, text).await;
        }

        let (a, b) = tokio::join!(fx.service.sync(&mut first), fx.service.sync(&mut second));
        let (mut a, mut b) = (a.unwrap(), b.unwrap());
        // Whoever was second adopts the rest in its next pass.
        a.confirmed.extend(fx.service.sync(&mut first).await.unwrap().confirmed);
        b.confirmed
            .extend(fx.service.sync(&mut second).await.unwrap().confirmed);

        assert_eq!(fx.stored_texts(&id).await, expected, "round {round}");
        for (name, outcome, state) in [("first", &a, &first), ("second", &b, &second)] {
            assert_eq!(
                versions(&outcome.confirmed),
                (1..=6).collect::<Vec<_>>(),
                "round {round} {name}"
            );
            assert_eq!(texts(&outcome.confirmed), expected, "round {round} {name}");
            assert_eq!(state.confirmed_version(), 6, "round {round} {name}");
        }
        assert_eq!(
            first.confirmed_chain_hash(),
            second.confirmed_chain_hash(),
            "round {round}"
        );
        fx.assert_log_is_sound(&id).await;
    }
}

// ---------------------------------------------------------------------------------------------
// Drift
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_log_contradicting_the_confirmed_history_makes_the_stream_reload() {
    let Some(fx) = fixture().await else { return };
    let id = stream_id();
    let seed = ChatLog::new("seed", 3);
    fx.log.db.ensure_stream(&id, KIND).await.unwrap();
    append_range(&fx.log.db, &id, &seed, 1, 3).await.unwrap();
    let mut state = fx.load(&id).await;
    // The log is replaced behind the instance's back by another history that is longer.
    fx.log
        .exec(&format!(
            "UPDATE cl_streams SET version = 0, chain_hash = 0 WHERE stream_id = '{id}';
             DELETE FROM cl_commands WHERE stream_id = '{id}';"
        ))
        .await;
    let other = ChatLog::new("other", 4);
    append_range(&fx.log.db, &id, &other, 1, 4).await.unwrap();
    fx.send(&id, "mine").await;

    let outcome = fx.service.sync(&mut state).await.unwrap();

    assert!(outcome.reload);
    assert!(outcome.confirmed.is_empty());
    assert_eq!(
        all_commands(&fx.log.db, &id).await,
        other.commands,
        "the log is left alone"
    );
}
