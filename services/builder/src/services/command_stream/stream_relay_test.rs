use crate::{
    models::messages::StreamMessage,
    repositories::{
        command_inbox::{redis::RedisCommandInboxDb, CommandInboxDb},
        command_log::{pg::PgCommandLogDb, CommandLogDb, StoredCommand},
    },
    services::command_stream::{PersistResult, StreamConfig, StreamLogService, StreamPersistence, StreamRelay},
    testing::{
        command_stream::{ChatLog, TestEgress, TestInbox, TestLog, TestStreamId},
        TestMessage,
    },
};
use shine_game::replica::{ChatState, StreamId};
use shine_test::test;
use uuid::Uuid;

type Relay = StreamRelay<ChatState, RedisCommandInboxDb, TestEgress>;
type Persistence = StreamPersistence<ChatState, RedisCommandInboxDb, PgCommandLogDb>;

/// A relay and the persistence behind it, with nothing spawned and no channel between them: the
/// relay only says what would have to be made durable, so the test hands that batch over itself.
/// The relay starts empty and takes its history from the first reload, as the worker does.
async fn setup(stream_id: &StreamId, inbox: &TestInbox, log: &TestLog) -> (Relay, Persistence) {
    setup_with(stream_id, inbox, log, StreamConfig::default()).await
}

async fn setup_with(
    stream_id: &StreamId,
    inbox: &TestInbox,
    log: &TestLog,
    config: StreamConfig,
) -> (Relay, Persistence) {
    let mut persistence = Persistence::new(
        stream_id.clone(),
        StreamLogService::new(inbox.db.clone(), log.db.clone(), StreamConfig::default()),
    );
    let mut relay = Relay::new(stream_id.clone(), inbox.db.clone(), TestEgress::default(), config);
    let PersistResult::Reloaded {
        snapshot, tail, stored_inbox, ..
    } = persistence.reload("the stream is starting").await
    else {
        panic!("expected the log to be read");
    };
    relay.accept_reload(snapshot, tail.to_vec(), stored_inbox);
    (relay, persistence)
}

/// Queues a command in the inbox of the stream, as the registry does after it has checked it.
async fn queue(inbox: &TestInbox, stream_id: &StreamId, data: &str) -> Uuid {
    let command_id = Uuid::new_v4();
    inbox
        .db
        .push(stream_id, command_id, Uuid::new_v4(), data)
        .await
        .unwrap();
    command_id
}

#[test]
async fn relay_submits_and_promotes_optimistic_commands() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let room = ChatLog::new("optimistic", 2);
    let (mut relay, mut persistence) = setup(&stream_id, &inbox, &log).await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    relay.subscribe(first, 0);
    relay.subscribe(second, 1);
    relay.subscribers.insert(first, 2);
    relay.optimistic.extend(room.commands.clone());

    let batch = relay.get_unpersisted().expect("optimistic commands are pending");
    assert!(
        !relay.optimistic.is_empty(),
        "building the batch leaves the commands optimistic: only a result confirms them"
    );
    let result = persistence
        .persist(batch.baseline_version, batch.baseline_chain_hash, batch.commands)
        .await;
    let PersistResult::Advanced { canonical, snapshot, .. } = result else {
        panic!("expected the optimistic batch to be confirmed");
    };
    assert!(
        !relay.accept_canonical(&canonical, snapshot),
        "the batch was confirmed exactly as the subscribers were told"
    );

    assert!(relay.optimistic.is_empty());
    assert_eq!(relay.confirmed, room.commands);
    assert_eq!(relay.subscribers.get(&first), Some(&2));
    assert_eq!(relay.subscribers.get(&second), Some(&1));
}

/// The commands of another instance may land between the submitted batch and the optimistic commands
/// that piled up behind it. Those are still the same commands, so they are kept rather than lost, but
/// they end up at versions other than the ones the subscribers were already told.
#[test]
async fn optimistic_commands_behind_a_confirmed_batch_are_reversioned() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let (mut relay, _persistence) = setup(&stream_id, &inbox, &log).await;

    // Two commands are submitted, two more pile up behind them while the job is in flight.
    let mine = ChatLog::new("mine", 4);
    relay.optimistic.extend(mine.commands.clone());
    let behind: Vec<_> = mine.commands[2..].iter().map(|c| c.command_id).collect();

    // The log confirms the first two, with a command of another instance after them.
    let other = ChatLog::new("other", 3);
    let mut canonical = mine.commands[..2].to_vec();
    canonical.push(StoredCommand {
        version: 3,
        ..other.commands[2].clone()
    });

    let resync = relay.accept_canonical(&canonical, None);

    assert!(resync, "the versions the subscribers were told do not hold");
    assert_eq!(
        relay.optimistic.iter().map(|c| c.command_id).collect::<Vec<_>>(),
        behind,
        "the commands behind the batch are kept, not dropped"
    );
    assert_eq!(
        relay.optimistic.iter().map(|c| c.version).collect::<Vec<_>>(),
        vec![4, 5],
        "they move after the command of the other instance"
    );
    assert_eq!(relay.head().0, 5);
}

/// A command sent twice is two inbox entries, so it is two commands with one id. Confirming the first
/// of them must not take the second with it.
#[test]
async fn a_command_queued_twice_is_confirmed_once_at_a_time() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let (mut relay, _persistence) = setup(&stream_id, &inbox, &log).await;

    // The same command arrived twice, so it is optimistic twice, at two versions.
    let room = ChatLog::new("twice", 1);
    let once = room.commands[0].clone();
    let twice = StoredCommand {
        version: 2,
        inbox_id: Some("1-2".to_string()),
        ..once.clone()
    };
    relay.optimistic.extend([once.clone(), twice]);

    // Only the first of them is stored.
    let resync = relay.accept_canonical(std::slice::from_ref(&once), None);

    assert!(!resync);
    assert_eq!(relay.confirmed.len(), 1, "one of the two is confirmed");
    assert_eq!(
        relay.optimistic.iter().map(|c| c.version).collect::<Vec<_>>(),
        vec![2],
        "the second one is still waiting, not dropped with the first"
    );
    assert_eq!(relay.optimistic[0].command_id, once.command_id);
}

#[test]
async fn relay_adopts_a_reload_and_invalidates_optimistic_history() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let (mut relay, mut persistence) = setup(&stream_id, &inbox, &log).await;
    let optimistic = ChatLog::new("mine", 2);
    relay.optimistic.extend(optimistic.commands.clone());

    let canonical = ChatLog::new("other", 2);
    canonical.persist(&log.db, &stream_id, 1, 2).await.unwrap();
    let commands = relay.optimistic.to_vec().into();

    let (version, chain_hash) = relay.confirmed_head();
    let result = persistence.persist(version, chain_hash, commands).await;
    let PersistResult::Reloaded {
        snapshot, tail, stored_inbox, ..
    } = result
    else {
        panic!("expected the losing optimistic history to reload");
    };
    relay.accept_reload(snapshot, tail.to_vec(), stored_inbox);

    assert!(relay.optimistic.is_empty());
    assert_eq!(relay.confirmed, canonical.commands);
    assert_eq!(
        log.db.head(&stream_id).await.unwrap().unwrap().chain_hash,
        canonical.replica.chain_hash()
    );
}

/// The registry validates a command before it is queued, but the inbox is shared by the instances and
/// outlives them, so the relay does not trust what it reads.
#[test]
async fn ingest_versions_the_inbox_in_order_and_drops_what_is_not_a_command() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let (mut relay, _persistence) = setup(&stream_id, &inbox, &log).await;
    let first = queue(&inbox, &stream_id, &TestMessage::new_chat_command("a")).await;
    queue(&inbox, &stream_id, "{not a command").await;
    queue(&inbox, &stream_id, &TestMessage::new_chat_command("   ")).await;
    let last = queue(&inbox, &stream_id, &TestMessage::new_chat_command("b")).await;

    let ingested = relay.ingest().await.unwrap();

    assert_eq!(ingested.commands.iter().map(|c| c.version).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(
        ingested.commands.iter().map(|c| c.command_id).collect::<Vec<_>>(),
        [first, last],
        "the dropped entries did not use a version"
    );
    assert!(!ingested.throttled);
    assert_eq!(relay.optimistic.len(), 2);

    assert!(
        relay.ingest().await.unwrap().commands.is_empty(),
        "the dropped entries are passed, not read again"
    );
    queue(&inbox, &stream_id, &TestMessage::new_chat_command("c")).await;
    assert_eq!(relay.ingest().await.unwrap().commands[0].version, 3);
}

#[test]
async fn ingest_leaves_the_inbox_to_wait_while_the_backlog_is_full() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let config = StreamConfig {
        max_pending: 3,
        ..StreamConfig::default()
    };
    let (mut relay, mut persistence) = setup_with(&stream_id, &inbox, &log, config).await;
    for text in ["a", "b", "c", "d", "e"] {
        queue(&inbox, &stream_id, &TestMessage::new_chat_command(text)).await;
    }

    let first = relay.ingest().await.unwrap();
    assert_eq!(first.commands.len(), 3, "no more than the backlog has room for");
    assert!(first.throttled);
    let stalled = relay.ingest().await.unwrap();
    assert!(stalled.commands.is_empty());
    assert!(
        stalled.throttled,
        "the full backlog is reported even when nothing was read"
    );

    let (version, chain_hash) = relay.confirmed_head();
    let PersistResult::Advanced { canonical, snapshot, .. } = persistence
        .persist(version, chain_hash, relay.optimistic.to_vec().into())
        .await
    else {
        panic!("expected the backlog to be confirmed");
    };
    relay.accept_canonical(&canonical, snapshot);

    let rest = relay.ingest().await.unwrap();
    assert_eq!(rest.commands.iter().map(|c| c.version).collect::<Vec<_>>(), [4, 5]);
    assert!(!rest.throttled, "nothing was held back any more");
}

#[test]
async fn deliver_sends_each_subscriber_only_what_it_has_not_seen() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let (mut relay, _persistence) = setup(&stream_id, &inbox, &log).await;
    for text in ["a", "b", "c"] {
        queue(&inbox, &stream_id, &TestMessage::new_chat_command(text)).await;
    }
    let ingested = relay.ingest().await.unwrap();
    let (behind, partway, current) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    relay.subscribe(behind, 0);
    relay.subscribe(partway, 1);
    relay.subscribe(current, 3);

    relay.deliver(&ingested.commands).await;

    let versions = |connection| TestEgress::versions(&relay.egress().messages(connection), &stream_id);
    assert_eq!(versions(behind), [1, 2, 3]);
    assert_eq!(versions(partway), [2, 3], "the command it has is not sent again");
    assert!(relay.egress().messages(current).is_empty(), "it has them all");
    assert_eq!(relay.subscribers.get(&partway), Some(&3));
    assert_eq!(relay.subscribers.get(&current), Some(&3));
}

#[test]
async fn giving_up_tells_the_subscribers_and_forgets_them() {
    let inbox = TestInbox::from_env().await;
    let log = TestLog::from_env().await;
    let stream_id = TestStreamId::new_chat();
    let (mut relay, _persistence) = setup(&stream_id, &inbox, &log).await;
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    relay.subscribe(first, 0);
    relay.subscribe(second, 0);

    relay.give_up().await;

    for connection in [first, second] {
        assert_eq!(
            relay
                .egress()
                .messages(connection)
                .iter()
                .map(|m| (**m).clone())
                .collect::<Vec<_>>(),
            [StreamMessage::Unsynced { stream_id: stream_id.clone() }]
        );
    }
    assert_eq!(relay.subscriber_count(), 0, "they have to subscribe again");
}
