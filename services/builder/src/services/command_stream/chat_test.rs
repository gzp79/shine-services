//! The chat kind end to end: users, the commands they send and what every replica ends up showing.

use super::{chat_access, CHAT_STREAM_ID};
use crate::{
    models::messages::StreamMessage,
    services::command_stream::{
        test_utils::{add, eventually, fast_cadence, setup, setup_with, start_worker, unique_id, Test},
        AllowAll, OpenError, StreamAccess, WorkerCommand,
    },
};
use shine_game::replay::{decode_command, decode_snapshot, ChatMessage, ChatState, CommandEnvelope, Replica};
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

#[test]
async fn another_stream_cannot_be_created_through_the_chat_access() {
    let Some(test) = setup(Arc::new(chat_access())).await else {
        return;
    };
    let stream_id = unique_id();
    assert!(matches!(
        test.service.open(Uuid::new_v4(), &stream_id, "chat").await,
        Err(OpenError::Forbidden)
    ));
    assert!(
        test.log.head(&stream_id).await.unwrap().is_none(),
        "nothing was created"
    );
}

/// What a replica shows for the stream, replayed from the snapshot route's payload.
async fn replayed(test: &Test, stream_id: &StreamId) -> Vec<ChatMessage> {
    let StreamMessage::Sync { snapshot, commands, .. } =
        test.service.read_snapshot(Uuid::nil(), stream_id).await.unwrap()
    else {
        panic!("expected a Sync");
    };
    let mut replica = match snapshot {
        Some(snapshot) => {
            Replica::from_snapshot(snapshot.version, decode_snapshot::<ChatState>(&snapshot.data).unwrap())
        }
        None => Replica::new(),
    };
    for command in commands.iter() {
        let envelope = CommandEnvelope {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            command: decode_command::<ChatState>(&command.data).unwrap(),
        };
        replica.apply_next(&envelope).unwrap();
    }
    replica.state().messages.iter().cloned().collect()
}

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

fn remove(id: Uuid) -> String {
    format!(r#"{{"type":"remove","id":"{id}"}}"#)
}

fn replace(id: Uuid, text: &str) -> String {
    format!(r#"{{"type":"replace","id":"{id}","text":"{text}"}}"#)
}

/// A stream with one message by `author`; returns its id.
async fn one_message(test: &Test, author: Uuid) -> (StreamId, Uuid) {
    let stream_id = unique_id();
    test.service.open(author, &stream_id, "chat").await.unwrap();
    let message_id = Uuid::new_v4();
    send(test, &stream_id, author, message_id, &add("hello")).await;
    (stream_id, message_id)
}

#[test]
async fn a_user_can_remove_their_own_message() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let author = Uuid::new_v4();
    let (stream_id, message_id) = one_message(&test, author).await;

    send(&test, &stream_id, author, Uuid::new_v4(), &remove(message_id)).await;
    assert!(replayed(&test, &stream_id).await.is_empty());
}

#[test]
async fn remove_of_another_users_message_is_stored_and_ignored() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let (stream_id, message_id) = one_message(&test, Uuid::new_v4()).await;

    let version = send(&test, &stream_id, Uuid::new_v4(), Uuid::new_v4(), &remove(message_id)).await;
    assert_eq!(version, 2, "the log grows by one");
    let messages = replayed(&test, &stream_id).await;
    assert_eq!(messages.len(), 1, "and the message stays");
    assert_eq!(messages[0].id, message_id);
}

#[test]
async fn remove_of_an_unknown_id_is_stored_and_ignored() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let author = Uuid::new_v4();
    let (stream_id, _) = one_message(&test, author).await;

    let version = send(&test, &stream_id, author, Uuid::new_v4(), &remove(Uuid::new_v4())).await;
    assert_eq!(version, 2);
    assert_eq!(replayed(&test, &stream_id).await.len(), 1);
}

#[test]
async fn a_user_can_replace_their_own_message() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let author = Uuid::new_v4();
    let (stream_id, message_id) = one_message(&test, author).await;

    send(
        &test,
        &stream_id,
        author,
        Uuid::new_v4(),
        &replace(message_id, "better"),
    )
    .await;
    let messages = replayed(&test, &stream_id).await;
    assert_eq!(messages.len(), 1);
    assert_eq!(
        (messages[0].id, messages[0].text.as_str(), messages[0].edited),
        (message_id, "better", true)
    );
}

#[test]
async fn replace_of_another_users_message_is_stored_and_ignored() {
    let Some(test) = setup(Arc::new(AllowAll)).await else {
        return;
    };
    let (stream_id, message_id) = one_message(&test, Uuid::new_v4()).await;

    let version = send(
        &test,
        &stream_id,
        Uuid::new_v4(),
        Uuid::new_v4(),
        &replace(message_id, "hijacked"),
    )
    .await;
    assert_eq!(version, 2);
    let messages = replayed(&test, &stream_id).await;
    assert_eq!((messages[0].text.as_str(), messages[0].edited), ("hello", false));
}

#[test]
async fn adding_a_chat_message_is_delivered_to_every_subscriber() {
    let Some(test) = setup_with(Arc::new(AllowAll), fast_cadence).await else {
        return;
    };
    let author = Uuid::new_v4();
    let stream_id = unique_id();
    test.service.open(author, &stream_id, "chat").await.unwrap();
    let running = start_worker(&test, &stream_id);
    let other = Uuid::new_v4();
    running
        .handle
        .tx
        .send(WorkerCommand::AddSubscriber(other, None))
        .unwrap();

    let message_id = Uuid::new_v4();
    send(&test, &stream_id, author, message_id, &add("to everyone")).await;

    eventually("both subscribers got the message", || async {
        running.egress.versions(running.connection_id) == [1] && running.egress.versions(other) == [1]
    })
    .await;
    for connection in [running.connection_id, other] {
        let commands = running.egress.commands(connection);
        assert_eq!((commands[0].command_id, commands[0].user_id), (message_id, author));
    }
    running.stop().await;
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
