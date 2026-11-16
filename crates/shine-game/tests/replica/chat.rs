use shine_game::replica::{
    stable_hash_of, ChatCommand, ChatMessage, ChatState, Command, ReplayState, CHAT_MAX_TEXT_BYTES,
};
use shine_test::test;
use uuid::Uuid;

#[test]
fn is_well_formed_add_text() {
    let add_text = |text: &str| ChatCommand::Add { text: text.to_string() };
    assert!(ChatState::is_well_formed(&add_text("hi")));
    assert!(!ChatState::is_well_formed(&add_text("")));
    assert!(!ChatState::is_well_formed(&add_text("  \n ")));
    assert!(ChatState::is_well_formed(&add_text(&"a".repeat(CHAT_MAX_TEXT_BYTES))));
    assert!(!ChatState::is_well_formed(&add_text(
        &"a".repeat(CHAT_MAX_TEXT_BYTES + 1)
    )));
    assert!(ChatState::is_well_formed(&ChatCommand::Remove { id: Uuid::nil() }));
}

#[test]
fn is_well_formed_replace_text() {
    let replace_text = |text: &str| ChatCommand::Replace {
        id: Uuid::nil(),
        text: text.to_string(),
    };
    assert!(ChatState::is_well_formed(&replace_text("fine")));
    assert!(!ChatState::is_well_formed(&replace_text(" ")));
    assert!(!ChatState::is_well_formed(&replace_text(
        &"a".repeat(CHAT_MAX_TEXT_BYTES + 1)
    )));
}

#[test]
fn chat_commands_has_stable_content_hash() {
    let id = Uuid::from_u128(9);

    assert_eq!(
        ChatCommand::Add { text: "hi".into() }.content_hash(),
        10797997847401506418
    );
    assert_eq!(ChatCommand::Remove { id }.content_hash(), 18384557926827126903);
    assert_eq!(
        ChatCommand::Replace { id, text: "new".into() }.content_hash(),
        8713239335255959125
    );
}

#[test]
fn chat_state_has_stable_hash() {
    let mut state = ChatState::default();
    state.messages.push_back(ChatMessage {
        id: Uuid::from_u128(1),
        user_id: Uuid::from_u128(7),
        text: "hi".into(),
        edited: false,
    });

    assert_eq!(stable_hash_of(&state), 2305080924836433373);
}
