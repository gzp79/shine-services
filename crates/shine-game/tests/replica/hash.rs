use crate::common::{command_id, user, Log};
use shine_game::replica::{stable_hash_of, ChatState, Replica, StableHasher};
use shine_test::test;

#[test]
fn fnv_matches_reference_vectors() {
    let mut h = StableHasher::new();
    assert_eq!(h.finish(), 0xcbf2_9ce4_8422_2325);
    h.write(b"a");
    assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn the_state_hash_is_the_count_then_each_message() {
    let mut replica = Replica::<ChatState>::new();
    let mut log = Log::new();
    replica.apply_next(&log.add(1, user(7), "hi")).unwrap();

    let mut h = StableHasher::new();
    h.write(&1u64.to_le_bytes());
    h.write(command_id(1).as_bytes());
    h.write(user(7).as_bytes());
    h.write(&2u64.to_le_bytes());
    h.write(b"hi");
    h.write(&[0u8]);
    assert_eq!(stable_hash_of(replica.state()), h.finish());
}

#[test]
fn the_hash_covers_the_author() {
    let mut a = Replica::<ChatState>::new();
    let mut b = Replica::<ChatState>::new();
    a.apply_next(&Log::new().add(1, user(1), "same")).unwrap();
    b.apply_next(&Log::new().add(1, user(2), "same")).unwrap();
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn the_same_log_gives_the_same_hash_on_two_replicas() {
    let mut log = Log::new();
    let entries: Vec<_> = (1..=10)
        .map(|v| log.add(v, user(v as u128 % 3), &format!("message {v}")))
        .collect();
    let mut a = Replica::<ChatState>::new();
    let mut b = Replica::<ChatState>::new();
    for e in &entries {
        a.apply_next(e).unwrap();
        b.apply_next(e).unwrap();
    }
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn the_hash_covers_the_edited_flag() {
    let mut a = Replica::<ChatState>::new();
    a.apply_next(&Log::new().add(1, user(1), "same")).unwrap();

    let mut b = Replica::<ChatState>::new();
    let mut log = Log::new();
    b.apply_next(&log.add(1, user(1), "same")).unwrap();
    // The same text, but replaced: the message is edited now.
    b.apply_next(&log.replace(2, user(1), command_id(1), "same")).unwrap();
    assert_ne!(a.hash(), b.hash());
}
