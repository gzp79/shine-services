use crate::demo_state::{envelope, set, user, DemoCommand, DemoState};
use shine_game::replay::{ApplyOutcome, CommandEnvelope, Gap, Replica};
use shine_test::test;
use uuid::Uuid;

#[test]
fn applied_and_ignored_commands_both_consume_a_version() {
    let mut replica = Replica::<DemoState>::new();
    assert_eq!(
        replica.apply_next(&envelope(1, user(1), set(0, 5))),
        Ok(ApplyOutcome::Applied)
    );
    let before = replica.hash();
    assert_eq!(
        replica.apply_next(&envelope(2, user(1), set(10, 1))),
        Ok(ApplyOutcome::Ignored)
    );
    assert_eq!(replica.version(), 2);
    assert_eq!(replica.hash(), before, "a rejected command leaves the state unchanged");
}

#[test]
fn undecodable_command_is_ignored() {
    let mut replica = Replica::<DemoState>::new();
    let undecodable = CommandEnvelope {
        version: 1,
        command_id: Uuid::nil(),
        user_id: Uuid::nil(),
        command: None,
    };
    assert_eq!(replica.apply_next(&undecodable), Ok(ApplyOutcome::Ignored));
    assert_eq!(replica.version(), 1);
}

#[test]
fn gap_and_already_applied_are_detected() {
    let mut replica = Replica::<DemoState>::new();
    assert_eq!(
        replica.apply_next(&envelope(2, user(1), set(0, 1))),
        Err(Gap { expected: 1, got: 2 })
    );
    replica.apply_next(&envelope(1, user(1), set(0, 1))).unwrap();
    assert_eq!(
        replica.apply_next(&envelope(1, user(1), set(0, 9))),
        Ok(ApplyOutcome::AlreadyApplied)
    );
    assert_eq!(replica.state().slots[0].value, 1);
}

#[test]
fn batch_is_all_or_nothing() {
    let mut replica = Replica::<DemoState>::new();
    let batch = DemoCommand::Batch {
        commands: vec![set(0, 1), set(1, 2), DemoCommand::Add { idx: 2, delta: 1 }, set(99, 0)],
    };
    assert_eq!(
        replica.apply_next(&envelope(1, user(1), batch)),
        Ok(ApplyOutcome::Ignored)
    );
    assert_eq!(replica.state(), &DemoState::default());
}

#[test]
fn the_replica_passes_the_command_context_to_apply() {
    let mut replica = Replica::<DemoState>::new();
    let e = envelope(1, user(7), set(3, 42));
    replica.apply_next(&e).unwrap();
    let slot = replica.state().slots[3];
    assert_eq!((slot.value, slot.writer, slot.command_id), (42, user(7), e.command_id));
}

#[test]
fn a_rejected_apply_leaves_the_state_unchanged_and_the_replay_continues() {
    let mut replica = Replica::<DemoState>::new();
    let write = envelope(1, user(1), set(0, 5));
    replica.apply_next(&write).unwrap();
    let before = replica.hash();

    let foreign_revert = envelope(
        2,
        user(2),
        DemoCommand::Revert {
            idx: 0,
            command_id: write.command_id,
        },
    );
    assert_eq!(replica.apply_next(&foreign_revert), Ok(ApplyOutcome::Ignored));
    assert_eq!(replica.hash(), before);

    assert_eq!(
        replica.apply_next(&envelope(3, user(1), set(1, 6))),
        Ok(ApplyOutcome::Applied)
    );
    assert_eq!(replica.version(), 3);
}
