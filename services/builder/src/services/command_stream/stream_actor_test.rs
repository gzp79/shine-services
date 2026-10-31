use crate::{
    repositories::{
        command_inbox::redis::RedisCommandInboxDb,
        command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand},
    },
    services::command_stream::{PersistResult, StreamActor, StreamConfig, StreamLogService, StreamPersistence},
    testing::command_stream::{
        create_test_inbox, create_test_log, persist_chat_logs, stream_id, ChatLog, TestEgress, TestInbox, TestLog,
    },
};
use shine_game::replica::{ChatState, ReplayState, StreamId};
use shine_test::test;
use uuid::Uuid;

type Actor = StreamActor<ChatState, RedisCommandInboxDb, TestEgress>;
type Persistence = StreamPersistence<ChatState, RedisCommandInboxDb, PgCommandLogDb>;

/// An actor and the persistence behind it, with nothing spawned and no channel between them: the
/// actor only says what would have to be made durable, so the test hands that batch over itself.
/// An actor and the persistence behind it, with nothing spawned and no channel between them: the
/// actor only says what would have to be made durable, so the test hands that batch over itself.
/// The actor starts empty and takes its history from the first reload, as the worker does.
async fn setup(stream_id: &StreamId, inbox: &TestInbox, log: &TestLog) -> (Actor, Persistence) {
    let mut persistence = Persistence::new(
        stream_id.clone(),
        StreamLogService::new(inbox.db.clone(), log.db.clone(), StreamConfig::default()),
    );
    let mut actor = Actor::new(
        stream_id.clone(),
        inbox.db.clone(),
        TestEgress::default(),
        StreamConfig::default(),
    );
    let PersistResult::Reloaded {
        snapshot, tail, stored_inbox, ..
    } = persistence.reload("the stream is starting").await
    else {
        panic!("expected the log to be read");
    };
    actor.accept_reload(snapshot, tail.to_vec(), stored_inbox);
    (actor, persistence)
}

#[test]
async fn actor_submits_and_promotes_optimistic_commands() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let Some(log) = create_test_log().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let room = ChatLog::new("optimistic", 2);
    let (mut actor, mut persistence) = setup(&stream_id, &inbox, &log).await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    actor.subscribe(first, 0);
    actor.subscribe(second, 1);
    actor.record_sent(first, 2);
    actor.append_optimistic(room.commands.clone());

    let commands = actor.optimistic().to_vec().into();
    assert!(
        !actor.optimistic().is_empty(),
        "building the batch leaves the commands optimistic: only a result confirms them"
    );
    let (version, chain_hash) = actor.confirmed_head();
    let result = persistence.persist(version, chain_hash, commands).await;
    let PersistResult::Advanced { canonical, snapshot, .. } = result else {
        panic!("expected the optimistic batch to be confirmed");
    };
    assert!(
        !actor.accept_canonical(&canonical, snapshot),
        "the batch was confirmed exactly as the subscribers were told"
    );

    assert!(actor.optimistic().is_empty());
    assert_eq!(actor.confirmed(), room.commands);
    assert_eq!(actor.sent_through(first), Some(2));
    assert_eq!(actor.sent_through(second), Some(1));
}

/// The commands of another instance may land between the submitted batch and the optimistic commands
/// that piled up behind it. Those are still the same commands, so they are kept rather than lost, but
/// they end up at versions other than the ones the subscribers were already told.
#[test]
async fn optimistic_commands_behind_a_confirmed_batch_are_reversioned() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let Some(log) = create_test_log().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let (mut actor, _persistence) = setup(&stream_id, &inbox, &log).await;

    // Two commands are submitted, two more pile up behind them while the job is in flight.
    let mine = ChatLog::new("mine", 4);
    actor.append_optimistic(mine.commands.clone());
    let behind: Vec<_> = mine.commands[2..].iter().map(|c| c.command_id).collect();

    // The log confirms the first two, with a command of another instance after them.
    let other = ChatLog::new("other", 3);
    let mut canonical = mine.commands[..2].to_vec();
    canonical.push(StoredCommand {
        version: 3,
        ..other.commands[2].clone()
    });

    let resync = actor.accept_canonical(&canonical, None);

    assert!(resync, "the versions the subscribers were told do not hold");
    assert_eq!(
        actor.optimistic().iter().map(|c| c.command_id).collect::<Vec<_>>(),
        behind,
        "the commands behind the batch are kept, not dropped"
    );
    assert_eq!(
        actor.optimistic().iter().map(|c| c.version).collect::<Vec<_>>(),
        vec![4, 5],
        "they move after the command of the other instance"
    );
    assert_eq!(actor.head_version(), 5);
}

/// A command sent twice is two inbox entries, so it is two commands with one id. Confirming the first
/// of them must not take the second with it.
#[test]
async fn a_command_queued_twice_is_confirmed_once_at_a_time() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let Some(log) = create_test_log().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let (mut actor, _persistence) = setup(&stream_id, &inbox, &log).await;

    // The same command arrived twice, so it is optimistic twice, at two versions.
    let room = ChatLog::new("twice", 1);
    let once = room.commands[0].clone();
    let twice = StoredCommand {
        version: 2,
        inbox_id: Some("1-2".to_string()),
        ..once.clone()
    };
    actor.append_optimistic([once.clone(), twice]);

    // Only the first of them is stored.
    let resync = actor.accept_canonical(std::slice::from_ref(&once), None);

    assert!(!resync);
    assert_eq!(actor.confirmed().len(), 1, "one of the two is confirmed");
    assert_eq!(
        actor.optimistic().iter().map(|c| c.version).collect::<Vec<_>>(),
        vec![2],
        "the second one is still waiting, not dropped with the first"
    );
    assert_eq!(actor.optimistic()[0].command_id, once.command_id);
}

#[test]
async fn actor_adopts_a_reload_and_invalidates_optimistic_history() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let Some(log) = create_test_log().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let (mut actor, mut persistence) = setup(&stream_id, &inbox, &log).await;
    let optimistic = ChatLog::new("mine", 2);
    actor.append_optimistic(optimistic.commands.clone());

    let canonical = ChatLog::new("other", 2);
    persist_chat_logs(&log.db, &stream_id, &canonical, 1, 2).await.unwrap();
    let commands = actor.optimistic().to_vec().into();

    let (version, chain_hash) = actor.confirmed_head();
    let result = persistence.persist(version, chain_hash, commands).await;
    let PersistResult::Reloaded {
        snapshot, tail, stored_inbox, ..
    } = result
    else {
        panic!("expected the losing optimistic history to reload");
    };
    actor.accept_reload(snapshot, tail.to_vec(), stored_inbox);

    assert!(actor.optimistic().is_empty());
    assert_eq!(actor.confirmed(), canonical.commands);
    assert_eq!(
        log.db.head(&stream_id).await.unwrap().unwrap().chain_hash,
        canonical.replica.chain_hash()
    );
}
