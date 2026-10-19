use crate::demo_state::{envelope, set, user, DemoCommand, DemoState};
use shine_game::replay::{decode_snapshot, encode_snapshot, stable_hash_of, CheckpointPolicy, Replica};
use shine_test::test;

#[test]
fn snapshot_round_trip_keeps_the_state_authors_and_hash() {
    let mut replica = Replica::<DemoState>::new();
    for v in 1..=10 {
        replica
            .apply_next(&envelope(v, user(v as u128), set((v % 10) as u32, i64::MIN + v)))
            .unwrap();
    }
    let bytes = encode_snapshot(replica.state()).unwrap();
    let decoded: DemoState = decode_snapshot(&bytes).unwrap();
    assert_eq!(&decoded, replica.state());
    assert_eq!(stable_hash_of(&decoded), replica.hash());
}

#[test]
fn replay_from_a_snapshot_equals_replay_from_zero() {
    let log: Vec<_> = (1..=20)
        .map(|v| {
            envelope(
                v,
                user(v as u128 % 4),
                DemoCommand::Add { idx: (v % 10) as u32, delta: v },
            )
        })
        .collect();

    let mut full = Replica::<DemoState>::new();
    for e in &log {
        full.apply_next(e).unwrap();
    }

    let mut prefix = Replica::<DemoState>::new();
    for e in &log[..10] {
        prefix.apply_next(e).unwrap();
    }
    let snapshot = encode_snapshot(prefix.state()).unwrap();
    let mut resumed = Replica::from_snapshot(10, decode_snapshot::<DemoState>(&snapshot).unwrap());
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
