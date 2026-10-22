use shine_game::replica::{stable_hash_of, StableHasher};
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
fn primitive_types_have_a_stable_hash() {
    assert_eq!(stable_hash_of("hi"), 11276750491067093148);
    assert_eq!(stable_hash_of(&42u64), 18391255480883862255);
    assert_eq!(stable_hash_of(&Uuid::from_u128(9)), 9808882566051098698);
}
