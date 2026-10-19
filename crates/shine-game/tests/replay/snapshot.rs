use crate::common::{add, command_id, remove, user};
use shine_game::replay::{
    decode_snapshot, encode_snapshot, stable_hash_of, ChatCommand, ChatState, CheckpointPolicy, CommandEnvelope,
    Replica,
};
use shine_test::test;

fn log(len: i64) -> Vec<CommandEnvelope<ChatCommand>> {
    (1..=len)
        .map(|v| {
            if v % 5 == 0 {
                remove(v, user(v as u128 % 3), command_id(v - 2))
            } else {
                add(v, user(v as u128 % 3), &format!("message {v} ✓"))
            }
        })
        .collect()
}

#[test]
fn a_snapshot_keeps_the_messages_the_authors_and_the_hash() {
    let mut replica = Replica::<ChatState>::new();
    for e in &log(10) {
        replica.apply_next(e).unwrap();
    }
    let bytes = encode_snapshot(replica.state()).unwrap();
    let decoded: ChatState = decode_snapshot(&bytes).unwrap();
    assert_eq!(&decoded, replica.state());
    assert_eq!(stable_hash_of(&decoded), replica.hash());
}

#[test]
fn replay_from_a_snapshot_equals_replay_from_zero() {
    let log = log(20);

    let mut full = Replica::<ChatState>::new();
    for e in &log {
        full.apply_next(e).unwrap();
    }

    let mut prefix = Replica::<ChatState>::new();
    for e in &log[..10] {
        prefix.apply_next(e).unwrap();
    }
    let snapshot = encode_snapshot(prefix.state()).unwrap();
    let mut resumed = Replica::from_snapshot(10, decode_snapshot::<ChatState>(&snapshot).unwrap());
    for e in &log[10..] {
        resumed.apply_next(e).unwrap();
    }
    assert_eq!(resumed.hash(), full.hash());
    assert_eq!(resumed.version(), 20);
}

#[test]
fn the_checkpoint_policy_fires_on_multiples() {
    let policy = CheckpointPolicy::new(5);
    assert!(!policy.is_checkpoint(0));
    assert!(!policy.is_checkpoint(4));
    assert!(policy.is_checkpoint(5));
    assert!(policy.is_checkpoint(10));
}
