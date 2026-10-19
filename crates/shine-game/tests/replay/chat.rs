use crate::common::{add, command_id, remove, replace, user};
use shine_game::replay::{ChatCommand, ChatState, ReplayState, Replica, CHAT_MAX_TEXT_BYTES, CHAT_WINDOW};
use shine_test::test;
use uuid::Uuid;

fn texts(replica: &Replica<ChatState>) -> Vec<&str> {
    replica.state().messages.iter().map(|m| m.text.as_str()).collect()
}

#[test]
fn add_appends_in_order_with_the_sender_and_the_command_id() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "hello")).unwrap();
    replica.apply_next(&add(2, user(2), "world")).unwrap();

    assert_eq!(texts(&replica), ["hello", "world"]);
    let first = &replica.state().messages[0];
    assert_eq!((first.id, first.user_id), (command_id(1), user(1)));
}

#[test]
fn the_window_keeps_the_last_messages() {
    let mut replica = Replica::<ChatState>::new();
    for v in 1..=CHAT_WINDOW as i64 + 1 {
        replica.apply_next(&add(v, user(1), &format!("m{v}"))).unwrap();
    }
    let messages = &replica.state().messages;
    assert_eq!(messages.len(), CHAT_WINDOW);
    assert_eq!(
        messages.front().unwrap().id,
        command_id(2),
        "the oldest message is gone"
    );
    assert_eq!(messages.back().unwrap().id, command_id(CHAT_WINDOW as i64 + 1));
}

#[test]
fn a_user_removes_their_own_message() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "a")).unwrap();
    replica.apply_next(&add(2, user(1), "b")).unwrap();
    replica.apply_next(&remove(3, user(1), command_id(1))).unwrap();
    assert_eq!(texts(&replica), ["b"]);
}

#[test]
fn removing_an_unknown_or_an_already_removed_message_is_ignored() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "a")).unwrap();
    let before = replica.hash();

    replica.apply_next(&remove(2, user(1), Uuid::from_u128(99))).unwrap();
    assert_eq!(replica.hash(), before);

    replica.apply_next(&remove(3, user(1), command_id(1))).unwrap();
    let after_remove = replica.hash();
    replica.apply_next(&remove(4, user(1), command_id(1))).unwrap();
    assert_eq!(replica.hash(), after_remove, "the second remove changes nothing");
    assert_eq!(replica.version(), 4);
}

#[test]
fn removing_another_users_message_is_ignored() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "a")).unwrap();
    let before = replica.hash();
    replica.apply_next(&remove(2, user(2), command_id(1))).unwrap();
    assert_eq!(replica.hash(), before);
    assert_eq!(texts(&replica), ["a"]);
}

#[test]
fn validate_checks_the_text() {
    let add_text = |text: &str| ChatCommand::Add { text: text.to_string() };
    assert!(ChatState::validate(&add_text("hi")));
    assert!(!ChatState::validate(&add_text("")));
    assert!(!ChatState::validate(&add_text("  \n ")));
    assert!(ChatState::validate(&add_text(&"a".repeat(CHAT_MAX_TEXT_BYTES))));
    assert!(!ChatState::validate(&add_text(&"a".repeat(CHAT_MAX_TEXT_BYTES + 1))));
    assert!(ChatState::validate(&ChatCommand::Remove { id: Uuid::nil() }));
}

#[test]
fn command_json_shape() {
    let add_json = r#"{"type":"add","text":"hi"}"#;
    assert_eq!(
        serde_json::from_str::<ChatCommand>(add_json).unwrap(),
        ChatCommand::Add { text: "hi".into() }
    );
    let remove_json = r#"{"type":"remove","id":"00000000-0000-0000-0000-000000000009"}"#;
    assert_eq!(
        serde_json::from_str::<ChatCommand>(remove_json).unwrap(),
        ChatCommand::Remove { id: Uuid::from_u128(9) }
    );
    assert!(serde_json::from_str::<ChatCommand>(r#"{"type":"add"}"#).is_err());
    assert!(serde_json::from_str::<ChatCommand>(r#"{"type":"edit","text":"x"}"#).is_err());
}

#[test]
fn a_new_message_is_not_edited() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "a")).unwrap();
    assert!(!replica.state().messages[0].edited);
}

#[test]
fn a_user_replaces_their_own_message_in_place_and_it_becomes_edited() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "first")).unwrap();
    replica.apply_next(&add(2, user(2), "second")).unwrap();
    replica
        .apply_next(&replace(3, user(1), command_id(1), "first, fixed"))
        .unwrap();

    assert_eq!(texts(&replica), ["first, fixed", "second"], "the position is kept");
    let edited: Vec<_> = replica.state().messages.iter().map(|m| m.edited).collect();
    assert_eq!(edited, [true, false]);
    assert_eq!(
        replica.state().messages[0].id,
        command_id(1),
        "the id stays the id of the add"
    );
}

#[test]
fn replacing_twice_keeps_the_message_edited_with_the_last_text() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "a")).unwrap();
    replica.apply_next(&replace(2, user(1), command_id(1), "b")).unwrap();
    replica.apply_next(&replace(3, user(1), command_id(1), "c")).unwrap();
    assert_eq!(texts(&replica), ["c"]);
    assert!(replica.state().messages[0].edited);
}

#[test]
fn replacing_an_unknown_or_a_removed_or_another_users_message_is_ignored() {
    let mut replica = Replica::<ChatState>::new();
    replica.apply_next(&add(1, user(1), "a")).unwrap();
    replica.apply_next(&add(2, user(1), "b")).unwrap();
    replica.apply_next(&remove(3, user(1), command_id(2))).unwrap();
    let before = replica.hash();

    replica
        .apply_next(&replace(4, user(1), Uuid::from_u128(99), "x"))
        .unwrap();
    replica.apply_next(&replace(5, user(1), command_id(2), "x")).unwrap();
    replica.apply_next(&replace(6, user(2), command_id(1), "x")).unwrap();

    assert_eq!(replica.hash(), before);
    assert_eq!(replica.version(), 6);
}

#[test]
fn a_replacement_text_is_validated_like_an_added_one() {
    let replace_text = |text: &str| ChatCommand::Replace {
        id: Uuid::nil(),
        text: text.to_string(),
    };
    assert!(ChatState::validate(&replace_text("fine")));
    assert!(!ChatState::validate(&replace_text(" ")));
    assert!(!ChatState::validate(&replace_text(
        &"a".repeat(CHAT_MAX_TEXT_BYTES + 1)
    )));
}

#[test]
fn the_replace_command_json_shape() {
    let json = r#"{"type":"replace","id":"00000000-0000-0000-0000-000000000009","text":"new"}"#;
    assert_eq!(
        serde_json::from_str::<ChatCommand>(json).unwrap(),
        ChatCommand::Replace {
            id: Uuid::from_u128(9),
            text: "new".into()
        }
    );
    let without_text = r#"{"type":"replace","id":"00000000-0000-0000-0000-000000000009"}"#;
    assert!(serde_json::from_str::<ChatCommand>(without_text).is_err());
}
