//! The chat kind end to end: users, the commands they send and what every replica ends up showing.

use super::{chat_access, CHAT_STREAM_ID};
use crate::{
    models::messages::StreamMessage,
    services::command_stream::{
        test_utils::{add, eventually, fast_cadence, setup_with, start_worker, unique_id, Test},
        AllowAll, StreamAccess, WorkerCommand,
    },
};
use shine_infra::db::command_log::StreamId;
use shine_test::test;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn only_the_global_chat_is_open_to_users() {
    let access = chat_access();
    let user = Uuid::new_v4();
    let chat = StreamId::parse(CHAT_STREAM_ID).unwrap();
    assert!(access.can_read(user, &chat) && access.can_write(user, &chat));
    for other in ["chat-other", "chat-global2", "x"] {
        let other = StreamId::parse(other).unwrap();
        assert!(
            !access.can_read(user, &other) && !access.can_write(user, &other),
            "{other}"
        );
    }
}

/// A chat message sent by `author`, through a direct ingest.
async fn send(test: &Test, stream_id: &StreamId, user: Uuid, command_id: Uuid, command: &str) -> i64 {
    let kind = test.service.kinds().get("chat").unwrap();
    match test
        .service
        .ingest(kind.as_ref(), stream_id, command_id, user, command)
        .await
        .unwrap()
    {
        crate::services::command_stream::IngestOutcome::Accepted(version) => version,
        other => panic!("not accepted: {other:?}"),
    }
}

#[test]
async fn a_joining_client_gets_the_snapshot_and_the_commands_after_it() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let stream_id = unique_id();
    test.service.open(Uuid::nil(), &stream_id, "chat").await.unwrap();
    let running = start_worker(&test, &stream_id);
    for i in 1..=12 {
        send(&test, &stream_id, Uuid::nil(), Uuid::new_v4(), &add(&format!("m{i}"))).await;
    }
    // Snapshots every 5: version 10 is the newest one once it is stored and pushed.
    eventually("the snapshot at 10", || async {
        running
            .egress
            .checkpoints(running.connection_id)
            .last()
            .map(|(v, _)| *v)
            == Some(10)
            && running.egress.versions(running.connection_id) == (1..=12).collect::<Vec<_>>()
    })
    .await;

    let joining = Uuid::new_v4();
    running
        .handle
        .tx
        .send(WorkerCommand::AddSubscriber(joining, None))
        .unwrap();
    eventually("the joining client's sync", || async {
        !running.egress.sent_to(joining).is_empty()
    })
    .await;
    let StreamMessage::Sync { snapshot, commands, .. } = running.egress.sent_to(joining).remove(0) else {
        panic!("the first message is a Sync");
    };
    assert_eq!(snapshot.map(|s| s.version), Some(10));
    assert_eq!(commands.iter().map(|c| c.version).collect::<Vec<_>>(), [11, 12]);
    running.stop().await;
}
