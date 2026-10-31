use super::*;
use crate::{
    repositories::{command_inbox::CommandInboxDb, command_log::CommandLogDb},
    services::command_stream::StreamConfig,
    testing::command_stream::{
        create_test_inbox, create_test_log, fetch_all_commands, seed_chat_log_and_snapshots, stream_id, ChatLog,
        TestInbox, TestLog,
    },
};
use shine_game::replica::{ChatState, ReplayState, StreamId};
use shine_test::test;

async fn fixture() -> Option<(TestInbox, TestLog)> {
    Some((create_test_inbox(60).await?, create_test_log().await?))
}

/// Nothing is spawned: the persistence is driven straight from the test.
fn persistence(
    stream_id: StreamId,
    inbox: &TestInbox,
    log: &TestLog,
    config: StreamConfig,
) -> StreamPersistence<ChatState, impl CommandInboxDb + 'static, impl CommandLogDb + 'static> {
    StreamPersistence::new(
        stream_id,
        StreamLogService::new(inbox.db.clone(), log.db.clone(), config),
    )
}

#[test]
async fn loading_takes_the_latest_snapshot_and_tail() {
    let Some((inbox, log)) = fixture().await else { return };
    let (stream_id, room) = seed_chat_log_and_snapshots(&log, 10, &[4, 7]).await;

    let loaded = persistence(stream_id, &inbox, &log, StreamConfig::default())
        .reload("the stream is starting")
        .await;

    let PersistResult::Reloaded { snapshot, tail, .. } = loaded else {
        panic!("expected the log to be read");
    };
    assert_eq!(snapshot.as_ref().map(|snapshot| snapshot.version), Some(7));
    assert_eq!(
        tail.iter().map(|command| command.version).collect::<Vec<_>>(),
        vec![8, 9, 10]
    );
    assert_eq!(&*tail, &room.commands[7..]);
}

#[test]
async fn persistence_appends_replays_and_returns_a_snapshot() {
    let Some((inbox, log)) = fixture().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let room = ChatLog::new("persist", 4);
    let config = StreamConfig {
        snapshot_every: 2,
        ..StreamConfig::default()
    };
    let mut persistence = persistence(stream_id.clone(), &inbox, &log, config);
    persistence.reload("the stream is starting").await;

    let result = persistence.persist(0, 0, room.commands.clone().into()).await;

    let PersistResult::Advanced {
        canonical,
        snapshot,
        checkpoint,
    } = result
    else {
        panic!("expected an advanced result");
    };
    assert_eq!(&*canonical, room.commands);
    assert_eq!(snapshot.as_ref().map(|value| value.version), Some(4));
    assert_eq!(checkpoint.map(|value| value.version), Some(4));
    assert_eq!(fetch_all_commands(&log.db, &stream_id).await, room.commands);
    assert_eq!(
        log.db
            .latest_snapshot(&stream_id, 4)
            .await
            .unwrap()
            .map(|value| value.version),
        Some(4)
    );
}

#[test]
async fn concurrent_instances_converge_on_the_same_canonical_batch() {
    let Some((inbox, log)) = fixture().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let room = ChatLog::new("shared", 6);
    let mut first = persistence(stream_id.clone(), &inbox, &log, StreamConfig::default());
    let mut second = persistence(stream_id.clone(), &inbox, &log, StreamConfig::default());
    first.reload("the stream is starting").await;
    second.reload("the stream is starting").await;

    let commands = || room.commands.clone().into();
    let (first_result, second_result) = tokio::join!(first.persist(0, 0, commands()), second.persist(0, 0, commands()));

    for result in [first_result, second_result] {
        let PersistResult::Advanced { canonical, .. } = result else {
            panic!("both instances must converge without a reload");
        };
        assert_eq!(&*canonical, room.commands);
    }
    assert_eq!(fetch_all_commands(&log.db, &stream_id).await, room.commands);
}

#[test]
async fn reconciliation_reloads_only_when_the_durable_head_changed() {
    let Some((inbox, log)) = fixture().await else { return };
    let stream_id = stream_id(ChatState::KIND);
    let mut watcher = persistence(stream_id.clone(), &inbox, &log, StreamConfig::default());
    let mut peer = persistence(stream_id.clone(), &inbox, &log, StreamConfig::default());
    watcher.reload("the stream is starting").await;
    peer.reload("the stream is starting").await;

    assert!(matches!(watcher.reconcile(0, 0).await, PersistResult::Current));

    let room = ChatLog::new("peer", 2);
    let result = peer.persist(0, 0, room.commands.clone().into()).await;
    assert!(matches!(result, PersistResult::Advanced { .. }));

    let PersistResult::Reloaded { tail, .. } = watcher.reconcile(0, 0).await else {
        panic!("the changed durable head must reload the canonical history");
    };
    assert_eq!(&*tail, room.commands);
}
