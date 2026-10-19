use crate::common::{add, command_id, remove, user};
use shine_game::replay::{stable_hash_of, ChatCommand, ChatState, CommandEnvelope, Replica};
use shine_test::test;

fn log(len: i64) -> Vec<CommandEnvelope<ChatCommand>> {
    (1..=len)
        .map(|v| {
            if v % 5 == 0 {
                remove(v, user(v as u128 % 3), command_id(v - 2))
            } else {
                add(v, user(v as u128 % 3), &format!("message {v} \u{2713}"))
            }
        })
        .collect()
}

#[test]
fn a_state_survives_a_serde_round_trip_with_the_same_hash() {
    let mut replica = Replica::<ChatState>::new();
    for e in &log(10) {
        replica.apply_next(e).unwrap();
    }
    let json = serde_json::to_string(replica.state()).unwrap();
    let decoded: ChatState = serde_json::from_str(&json).unwrap();
    assert_eq!(&decoded, replica.state());
    assert_eq!(stable_hash_of(&decoded), replica.hash());
}

#[test]
fn replay_from_a_state_equals_replay_from_zero() {
    let log = log(20);

    let mut full = Replica::<ChatState>::new();
    for e in &log {
        full.apply_next(e).unwrap();
    }

    let mut prefix = Replica::<ChatState>::new();
    for e in &log[..10] {
        prefix.apply_next(e).unwrap();
    }
    let mut resumed = Replica::from_snapshot(10, prefix.state().clone());
    for e in &log[10..] {
        resumed.apply_next(e).unwrap();
    }
    assert_eq!(resumed.hash(), full.hash());
    assert_eq!(resumed.version(), 20);
}

#[test]
fn a_replica_resumed_from_a_state_rejects_older_versions() {
    let log = log(6);
    let mut prefix = Replica::<ChatState>::new();
    for e in &log[..3] {
        prefix.apply_next(e).unwrap();
    }
    let mut resumed = Replica::from_snapshot(3, prefix.state().clone());
    assert_eq!(
        resumed.apply_next(&log[1]),
        Ok(shine_game::replay::ApplyOutcome::AlreadyApplied)
    );
    assert_eq!(
        resumed.apply_next(&log[3]),
        Ok(shine_game::replay::ApplyOutcome::Applied)
    );
}
