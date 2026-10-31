use crate::{
    repositories::command_inbox::{CommandInboxDb, InboxEntry, InboxId, InboxWake},
    testing::command_stream::{create_test_inbox, stream_id, wait_inbox_wake},
};
use shine_game::replica::StreamKind;
use shine_test::test;
use uuid::Uuid;

fn entry(inbox_id: &InboxId, command_id: Uuid, user_id: Uuid, data: &str) -> InboxEntry {
    InboxEntry {
        inbox_id: inbox_id.clone(),
        command_id,
        user_id,
        data: data.to_string(),
    }
}

// ---------------------------------------------------------------------------------------------
// Pushing and polling
// ---------------------------------------------------------------------------------------------

#[test]
async fn pushed_commands_are_polled_in_order_with_all_their_fields() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let id = stream_id(StreamKind::Chat);
    let (first_command, first_user) = (Uuid::new_v4(), Uuid::new_v4());
    let (second_command, second_user) = (Uuid::new_v4(), Uuid::new_v4());

    let first = inbox
        .db
        .push(&id, first_command, first_user, r#"{"text":"a b"}"#)
        .await
        .unwrap();
    let second = inbox.db.push(&id, second_command, second_user, "").await.unwrap();
    let batch = inbox.db.poll(&id, &InboxId::START, 10).await.unwrap();

    assert!(first < second);
    assert_eq!(
        batch.entries,
        vec![
            entry(&first, first_command, first_user, r#"{"text":"a b"}"#),
            entry(&second, second_command, second_user, ""),
        ]
    );
    assert_eq!(batch.cursor, second);
}

#[test]
async fn polling_continues_after_the_cursor_in_pages_and_stops_at_the_end() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let id = stream_id(StreamKind::Chat);
    let mut pushed = Vec::new();
    for i in 0..5 {
        pushed.push(
            inbox
                .db
                .push(&id, Uuid::new_v4(), Uuid::new_v4(), &format!("{i}"))
                .await
                .unwrap(),
        );
    }

    let mut cursor = InboxId::START;
    let mut polled = Vec::new();
    for (expected_len, expected_more) in [(2, true), (2, true), (1, false)] {
        let batch = inbox.db.poll(&id, &cursor, 2).await.unwrap();
        assert_eq!(batch.entries.len(), expected_len);
        assert_eq!(batch.more, expected_more);
        polled.extend(batch.entries.iter().map(|e| e.inbox_id.clone()));
        cursor = batch.cursor;
    }
    let end = inbox.db.poll(&id, &cursor, 2).await.unwrap();

    assert_eq!(polled, pushed);
    assert_eq!(cursor, pushed[4]);
    assert!(end.entries.is_empty());
    assert_eq!(end.cursor, cursor, "nothing new leaves the cursor where it was");
}

#[test]
async fn a_stream_without_an_inbox_polls_empty_and_streams_do_not_see_each_other() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let (first, second, missing) = (
        stream_id(StreamKind::Chat),
        stream_id(StreamKind::Chat),
        stream_id(StreamKind::Chat),
    );
    let in_first = inbox
        .db
        .push(&first, Uuid::new_v4(), Uuid::new_v4(), "first")
        .await
        .unwrap();
    inbox
        .db
        .push(&second, Uuid::new_v4(), Uuid::new_v4(), "second")
        .await
        .unwrap();

    let empty = inbox.db.poll(&missing, &InboxId::START, 10).await.unwrap();
    let only_first = inbox.db.poll(&first, &InboxId::START, 10).await.unwrap();

    assert!(empty.entries.is_empty());
    assert_eq!(empty.cursor, InboxId::START);
    assert_eq!(only_first.entries.len(), 1);
    assert_eq!(only_first.entries[0].inbox_id, in_first);
}

#[test]
async fn entries_that_cannot_be_read_are_skipped_but_the_cursor_moves_past_them() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let id = stream_id(StreamKind::Chat);
    let (command, user) = (Uuid::new_v4(), Uuid::new_v4());
    let before = inbox.db.push(&id, command, user, "before").await.unwrap();
    inbox.raw_add(&id, &[("data", "no ids")]).await;
    let after = inbox.db.push(&id, command, user, "after").await.unwrap();
    inbox
        .raw_add(
            &id,
            &[
                ("command_id", "not-a-uuid"),
                ("user_id", &user.to_string()),
                ("data", "x"),
            ],
        )
        .await;
    let last_poison = inbox.raw_add(&id, &[("unrelated", "field")]).await;

    let batch = inbox.db.poll(&id, &InboxId::START, 10).await.unwrap();
    let again = inbox.db.poll(&id, &batch.cursor, 10).await.unwrap();

    let polled: Vec<_> = batch.entries.iter().map(|e| e.inbox_id.clone()).collect();
    assert_eq!(polled, vec![before, after]);
    assert_eq!(batch.cursor, last_poison, "a poison entry at the end is not read again");
    assert!(again.entries.is_empty());
}

// ---------------------------------------------------------------------------------------------
// Trimming and expiry
// ---------------------------------------------------------------------------------------------

#[test]
async fn trimming_removes_the_entries_up_to_and_including_the_given_one() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let id = stream_id(StreamKind::Chat);
    let mut pushed = Vec::new();
    for i in 0..5 {
        pushed.push(
            inbox
                .db
                .push(&id, Uuid::new_v4(), Uuid::new_v4(), &format!("{i}"))
                .await
                .unwrap(),
        );
    }

    let removed = inbox.db.trim(&id, &pushed[2]).await.unwrap();

    let rest = inbox.db.poll(&id, &InboxId::START, 10).await.unwrap();
    assert_eq!(removed, 3);
    assert_eq!(
        rest.entries.iter().map(|e| e.inbox_id.clone()).collect::<Vec<_>>(),
        pushed[3..]
    );
}

#[test]
async fn trimming_is_monotonic_and_a_trimmed_inbox_keeps_accepting_commands() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let id = stream_id(StreamKind::Chat);
    let mut pushed = Vec::new();
    for i in 0..4 {
        pushed.push(
            inbox
                .db
                .push(&id, Uuid::new_v4(), Uuid::new_v4(), &format!("{i}"))
                .await
                .unwrap(),
        );
    }

    assert_eq!(inbox.db.trim(&id, &pushed[1]).await.unwrap(), 2);
    assert_eq!(
        inbox.db.trim(&id, &pushed[0]).await.unwrap(),
        0,
        "a lower trim point removes nothing"
    );
    assert_eq!(inbox.db.trim(&id, &pushed[3]).await.unwrap(), 2, "everything can go");
    assert_eq!(inbox.len(&id).await, 0);

    let later = inbox
        .db
        .push(&id, Uuid::new_v4(), Uuid::new_v4(), "later")
        .await
        .unwrap();
    let batch = inbox.db.poll(&id, &pushed[3], 10).await.unwrap();
    assert_eq!(
        batch.entries.iter().map(|e| e.inbox_id.clone()).collect::<Vec<_>>(),
        vec![later]
    );
}

#[test]
async fn trimming_a_stream_without_an_inbox_removes_nothing() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };

    let removed = inbox
        .db
        .trim(
            &stream_id(StreamKind::Chat),
            &InboxId::parse("99999999999999-0").unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(removed, 0);
}

#[test]
async fn every_push_renews_the_expiry_of_the_inbox() {
    let Some(inbox) = create_test_inbox(600).await else {
        return;
    };
    let id = stream_id(StreamKind::Chat);

    inbox.db.push(&id, Uuid::new_v4(), Uuid::new_v4(), "a").await.unwrap();
    let after_first = inbox.ttl(&id).await;
    inbox.set_ttl(&id, 5).await;
    inbox.db.push(&id, Uuid::new_v4(), Uuid::new_v4(), "b").await.unwrap();
    let after_second = inbox.ttl(&id).await;

    assert!((1..=600).contains(&after_first), "{after_first}");
    assert!((500..=600).contains(&after_second), "{after_second}");
}

// ---------------------------------------------------------------------------------------------
// Wakes
// ---------------------------------------------------------------------------------------------
#[test]
async fn a_push_wakes_the_listeners_of_its_stream() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let mut wakes = inbox.listen().await;
    let id = stream_id(StreamKind::Chat);

    inbox.db.push(&id, Uuid::new_v4(), Uuid::new_v4(), "a").await.unwrap();

    let wake = wait_inbox_wake(&mut wakes, &id).await;
    assert_eq!(wake, InboxWake::Stream { stream_id: id, persisted: None });
}

#[test]
async fn announcing_a_persisted_version_wakes_the_listeners_with_it() {
    let Some(inbox) = create_test_inbox(60).await else {
        return;
    };
    let mut wakes = inbox.listen().await;
    let id = stream_id(StreamKind::Chat);

    inbox.db.notify_persisted(&id, 42).await.unwrap();

    let wake = wait_inbox_wake(&mut wakes, &id).await;
    assert_eq!(
        wake,
        InboxWake::Stream {
            stream_id: id,
            persisted: Some(42)
        }
    );
}
