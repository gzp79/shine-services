use crate::demo_state::{context_of, envelope, set, user, DemoCommand, DemoReject, DemoState, MAX_BATCH_LEN};
use shine_game::replay::ReplayState;
use shine_test::test;
use uuid::Uuid;

fn apply(state: &mut DemoState, user_id: Uuid, command: DemoCommand) -> Result<(), DemoReject> {
    let ctx = context_of(&envelope(1, user_id, command.clone()));
    state.apply(&ctx, &command)
}

#[test]
fn commands_apply_and_reject() {
    let mut s = DemoState::default();
    apply(&mut s, user(1), set(3, 7)).unwrap();
    apply(&mut s, user(1), DemoCommand::Swap { a: 3, b: 4 }).unwrap();
    assert_eq!((s.slots[3].value, s.slots[4].value), (0, 7));
    let wrong = DemoCommand::CompareAndSet { idx: 4, expected: 1, value: 2 };
    assert_eq!(apply(&mut s, user(1), wrong), Err(DemoReject::Mismatch));
    let right = DemoCommand::CompareAndSet { idx: 4, expected: 7, value: 2 };
    apply(&mut s, user(1), right).unwrap();
    assert_eq!(s.slots[4].value, 2);
    apply(&mut s, user(1), set(0, i64::MAX)).unwrap();
    assert_eq!(
        apply(&mut s, user(1), DemoCommand::Add { idx: 0, delta: 1 }),
        Err(DemoReject::Overflow)
    );
    assert_eq!(apply(&mut s, user(1), set(10, 1)), Err(DemoReject::IndexOutOfRange(10)));
}

#[test]
fn revert_by_the_author_clears_the_slot() {
    let mut s = DemoState::default();
    let write = envelope(1, user(1), set(2, 9));
    s.apply(&context_of(&write), write.command.as_ref().unwrap()).unwrap();
    let revert = DemoCommand::Revert {
        idx: 2,
        command_id: write.command_id,
    };
    apply(&mut s, user(1), revert).unwrap();
    assert_eq!(s, DemoState::default());
}

#[test]
fn revert_of_an_unknown_command_is_rejected() {
    let mut s = DemoState::default();
    apply(&mut s, user(1), set(2, 9)).unwrap();
    let revert = DemoCommand::Revert {
        idx: 2,
        command_id: Uuid::from_u128(99),
    };
    assert_eq!(apply(&mut s, user(1), revert), Err(DemoReject::NotFound));
}

#[test]
fn revert_by_another_user_is_rejected_and_changes_nothing() {
    let mut s = DemoState::default();
    let write = envelope(1, user(1), set(2, 9));
    s.apply(&context_of(&write), write.command.as_ref().unwrap()).unwrap();
    let before = s.clone();
    let revert = DemoCommand::Revert {
        idx: 2,
        command_id: write.command_id,
    };
    assert_eq!(apply(&mut s, user(2), revert), Err(DemoReject::NotOwner));
    assert_eq!(s, before);
}

#[test]
fn validate_rejects_nested_and_long_batches() {
    let nested = DemoCommand::Batch {
        commands: vec![DemoCommand::Batch { commands: vec![] }],
    };
    assert!(!DemoState::validate(&nested));
    let long = DemoCommand::Batch {
        commands: (0..=MAX_BATCH_LEN as u32).map(|i| set(i % 10, 1)).collect(),
    };
    assert!(!DemoState::validate(&long));
    assert!(DemoState::validate(&DemoCommand::Batch { commands: vec![set(0, 1)] }));
}

#[test]
fn validate_rejects_a_revert_inside_a_batch() {
    let batch = DemoCommand::Batch {
        commands: vec![DemoCommand::Revert {
            idx: 0,
            command_id: Uuid::nil(),
        }],
    };
    assert!(!DemoState::validate(&batch));
}

#[test]
fn refers_to_names_the_reverted_command_only() {
    let target = Uuid::from_u128(9);
    let revert = DemoCommand::Revert { idx: 0, command_id: target };
    assert_eq!(DemoState::refers_to(&revert), Some(target));
    assert_eq!(DemoState::refers_to(&set(0, 1)), None);
    assert_eq!(
        DemoState::refers_to(&DemoCommand::Batch { commands: vec![revert] }),
        None
    );
}

#[test]
fn command_json_shape() {
    let cmd: DemoCommand = serde_json::from_str(r#"{"op":"compareAndSet","idx":1,"expected":2,"value":3}"#).unwrap();
    assert_eq!(cmd, DemoCommand::CompareAndSet { idx: 1, expected: 2, value: 3 });
    let revert = r#"{"op":"revert","idx":1,"commandId":"00000000-0000-0000-0000-000000000009"}"#;
    assert_eq!(
        serde_json::from_str::<DemoCommand>(revert).unwrap(),
        DemoCommand::Revert {
            idx: 1,
            command_id: Uuid::from_u128(9)
        }
    );
    let snake = r#"{"op":"revert","idx":1,"command_id":"00000000-0000-0000-0000-000000000009"}"#;
    assert!(serde_json::from_str::<DemoCommand>(snake).is_err());
    assert!(serde_json::from_str::<DemoCommand>(r#"{"op":"set","idx":"x","value":1}"#).is_err());
}
