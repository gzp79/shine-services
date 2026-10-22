use crate::common::{command_id, user, Log};
use shine_game::replica::{ApplyError, ApplyOutcome, ChatState, Replica};
use shine_test::test;

#[test]
fn the_state_never_sees_a_gap_or_a_repeat() {
    let mut replica = Replica::<ChatState>::new();
    // Its own, throwaway log: rejected on the gap check, before any chain check, so it never really
    // advances the chain the real sequence below starts fresh from.
    assert_eq!(
        replica.apply_next(&Log::new().add(2, user(1), "late")),
        Err(ApplyError::Gap { expected: 1, got: 2 })
    );
    assert_eq!(replica.version(), 0);
    assert_eq!(replica.state(), &ChatState::default(), "a gap must not reach the state");

    let mut log = Log::new();
    assert_eq!(
        replica.apply_next(&log.add(1, user(1), "first")),
        Ok(ApplyOutcome::Applied)
    );
    assert_eq!(
        replica.apply_next(&log.add(1, user(1), "again")),
        Ok(ApplyOutcome::Skipped)
    );
    assert_eq!(replica.state().messages.len(), 1, "a repeat must not reach the state");
}

#[test]
fn a_command_the_state_ignores_still_consumes_its_version() {
    let mut replica = Replica::<ChatState>::new();
    let mut log = Log::new();
    replica.apply_next(&log.add(1, user(1), "a")).unwrap();
    let before = replica.hash();

    // Another user's remove: the state ignores it, the runtime sees a normal apply.
    assert_eq!(
        replica.apply_next(&log.remove(2, user(2), command_id(1))),
        Ok(ApplyOutcome::Applied)
    );
    assert_eq!(replica.version(), 2);
    assert_eq!(replica.hash(), before);

    assert_eq!(replica.apply_next(&log.add(3, user(1), "b")), Ok(ApplyOutcome::Applied));
    assert_eq!(replica.version(), 3);
}

#[test]
fn a_checkpoint_names_the_version_and_the_hash() {
    let mut replica = Replica::<ChatState>::new();
    let mut log = Log::new();
    replica.apply_next(&log.add(1, user(1), "a")).unwrap();
    let checkpoint = replica.checkpoint();
    assert_eq!((checkpoint.version, checkpoint.state_hash), (1, replica.hash()));
}
