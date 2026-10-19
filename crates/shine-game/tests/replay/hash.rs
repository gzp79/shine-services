use crate::demo_state::{envelope, set, user, DemoState, SLOTS};
use shine_game::replay::{stable_hash_of, Replica, StableHasher};
use shine_test::test;
use uuid::Uuid;

#[test]
fn fnv_matches_reference_vectors() {
    let mut h = StableHasher::new();
    assert_eq!(h.finish(), 0xcbf2_9ce4_8422_2325);
    h.write(b"a");
    assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn state_hash_is_length_then_the_slots() {
    let mut replica = Replica::<DemoState>::new();
    let e = envelope(1, user(7), set(0, -1));
    replica.apply_next(&e).unwrap();

    let mut h = StableHasher::new();
    h.write(&(SLOTS as u64).to_le_bytes());
    h.write(&(-1i64).to_le_bytes());
    h.write(user(7).as_bytes());
    h.write(e.command_id.as_bytes());
    for _ in 1..SLOTS {
        h.write(&0i64.to_le_bytes());
        h.write(Uuid::nil().as_bytes());
        h.write(Uuid::nil().as_bytes());
    }
    assert_eq!(stable_hash_of(replica.state()), h.finish());
}

#[test]
fn the_hash_covers_the_author() {
    let mut a = Replica::<DemoState>::new();
    let mut b = Replica::<DemoState>::new();
    let mut e = envelope(1, user(1), set(0, 3));
    a.apply_next(&e).unwrap();
    e.user_id = user(2);
    b.apply_next(&e).unwrap();
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn the_same_log_gives_the_same_hash_on_two_replicas() {
    let log: Vec<_> = (1..=10)
        .map(|v| envelope(v, user(v as u128 % 3), set((v % 10) as u32, v)))
        .collect();
    let mut a = Replica::<DemoState>::new();
    let mut b = Replica::<DemoState>::new();
    for e in &log {
        a.apply_next(e).unwrap();
        b.apply_next(e).unwrap();
    }
    assert_eq!(a.hash(), b.hash());
}
