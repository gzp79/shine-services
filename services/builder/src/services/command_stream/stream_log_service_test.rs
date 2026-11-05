use super::*;
use crate::{
    repositories::{
        command_inbox::{CommandInboxDb, InboxId},
        command_log::CommandLogDb,
    },
    testing::command_stream::{TestInbox, TestLog, TestStreamId},
};

use shine_test::test;

async fn fixture() -> (TestInbox, TestLog) {
    (TestInbox::from_env().await, TestLog::from_env().await)
}

fn service(inbox: &TestInbox, log: &TestLog) -> StreamLogService<impl CommandInboxDb, impl CommandLogDb> {
    StreamLogService::new(inbox.db.clone(), log.db.clone(), StreamConfig::default())
}

#[test]
async fn a_new_stream_loads_empty_and_is_created() {
    let (inbox, log) = fixture().await;
    let id = TestStreamId::new_chat();

    let stored = service(&inbox, &log).load(&id).await.unwrap();

    assert!(stored.snapshot.is_none());
    assert!(stored.tail.is_empty());
    assert!(
        stored.stored_inbox.is_none(),
        "nothing is stored, so no inbox entry may be skipped"
    );
    assert!(log.db.head(&id).await.unwrap().is_some(), "loading creates the stream");
}

#[test]
async fn a_stream_without_a_snapshot_loads_every_command() {
    let (inbox, log) = fixture().await;
    let (id, room) = log.seeded_chat_log(3).await;

    let stored = service(&inbox, &log).load(&id).await.unwrap();

    assert!(stored.snapshot.is_none());
    assert_eq!(
        stored.tail, room.commands,
        "reading from version 0 keeps the first command"
    );
}

#[test]
async fn a_stream_loads_from_its_latest_snapshot_without_the_command_below_it() {
    let (inbox, log) = fixture().await;
    let (id, room) = log.seed_chat_log_and_snapshots(10, &[4, 7]).await;

    let stored = service(&inbox, &log).load(&id).await.unwrap();

    assert_eq!(stored.snapshot.map(|snapshot| snapshot.version), Some(7));
    assert_eq!(
        stored.tail,
        room.commands[7..],
        "the command at the snapshot's version is read but stripped"
    );
}

#[test]
async fn the_stored_inbox_marker_comes_from_the_last_command_of_the_tail() {
    let (inbox, log) = fixture().await;
    let (id, room) = log.seed_chat_log_and_snapshots(10, &[7]).await;

    let stored = service(&inbox, &log).load(&id).await.unwrap();

    let expected = room.commands[9].inbox_id.as_deref().and_then(InboxId::parse);
    assert!(expected.is_some(), "the seeded commands carry an inbox id");
    assert_eq!(stored.stored_inbox, expected);
}

/// Every command after the snapshot may be pruned away, and the marker then has to come from the
/// command the snapshot was taken at: its inbox entry is stored and must not be taken again.
#[test]
async fn the_stored_inbox_marker_survives_an_empty_tail() {
    let (inbox, log) = fixture().await;
    let (id, room) = log.seed_chat_log_and_snapshots(5, &[5]).await;

    let stored = service(&inbox, &log).load(&id).await.unwrap();

    assert!(stored.tail.is_empty(), "the snapshot covers every command");
    assert_eq!(
        stored.stored_inbox,
        room.commands[4].inbox_id.as_deref().and_then(InboxId::parse)
    );
}
