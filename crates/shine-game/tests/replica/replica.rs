use crate::command_log::{command_id, user, ChatCommandLog};
use shine_game::replica::{
    decode_snapshot, encode_snapshot, ApplyError, ApplyOutcome, ChatCommand, ChatState, CommandEnvelope, Replica,
};
use shine_test::test;

#[test]
fn a_replica_resumed_from_a_snapshot_replays_to_the_same_state_as_from_scratch() {
    let log = ChatCommandLog::sample(20);
    let entries = log.entries();
    let mut full = Replica::<ChatState>::new();
    let mut prefix = Replica::<ChatState>::new();
    for (i, e) in entries.iter().enumerate() {
        full.apply_next(e).unwrap();
        if i < 10 {
            prefix.apply_next(e).unwrap();
        }
    }

    // What a resumed replica is built from: a decoded snapshot of the state, and the chain hash
    // through it (from a `Checkpoint` taken at the same version, not recomputed here).
    let bytes = encode_snapshot(prefix.state()).unwrap();
    let decoded = decode_snapshot::<ChatState>(&bytes).unwrap();
    assert_eq!(&decoded, prefix.state());

    let mut resumed = Replica::from_snapshot(10, decoded, prefix.chain_hash());
    for e in &entries[10..] {
        resumed.apply_next(e).unwrap();
    }
    assert_eq!(
        resumed.hash(),
        full.hash(),
        "the state matches a full replay from version 0"
    );
    assert_eq!(
        resumed.chain_hash(),
        full.chain_hash(),
        "the chain hash matches a full replay from version 0"
    );
}

#[test]
fn a_gap_is_rejected_before_reaching_the_state() {
    let mut replica = Replica::<ChatState>::new();
    // A version that does not follow from a fresh replica; the chain hash does not matter here, the
    // gap check rejects it before any chain check is even reached.
    let late = CommandEnvelope::new(2, command_id(2), user(1), 0, ChatCommand::Add { text: "late".into() });
    assert_eq!(replica.apply_next(&late), Err(ApplyError::Gap { expected: 1, got: 2 }));
    assert_eq!(replica.version(), 0, "a gap must not reach the state");
}

#[test]
fn a_repeat_is_skipped_before_reaching_the_state() {
    let mut replica = Replica::<ChatState>::new();
    let mut log = ChatCommandLog::default();
    let first = log.add(user(1), "first").clone();
    assert_eq!(replica.apply_next(&first), Ok(ApplyOutcome::Applied));
    // The same envelope delivered again, as a retry would.
    assert_eq!(replica.apply_next(&first), Ok(ApplyOutcome::Skipped));
    assert_eq!(replica.state().messages.len(), 1, "a repeat must not reach the state");
}

// The chain hash is what the log, the clients and the snapshots agree on: the same content from
// another author, or under another command id, has to be another history.
#[test]
fn the_same_content_from_another_author_or_command_id_has_another_chain_hash() {
    let command = || ChatCommand::Add { text: "gg".into() };
    let original = CommandEnvelope::new(1, command_id(1), user(1), 0, command());
    let other_author = CommandEnvelope::new(1, command_id(1), user(2), 0, command());
    let other_command_id = CommandEnvelope::new(1, command_id(2), user(1), 0, command());

    assert_eq!(
        original.command_hash, other_author.command_hash,
        "the content is the same"
    );
    assert_ne!(original.header.chain_hash, other_author.header.chain_hash);
    assert_ne!(original.header.chain_hash, other_command_id.header.chain_hash);
}

#[test]
fn a_command_of_another_author_with_the_chain_hash_of_the_original_is_rejected_as_diverged() {
    let command = || ChatCommand::Add { text: "gg".into() };
    let original = CommandEnvelope::new(1, command_id(1), user(1), 0, command());
    let mut forged = CommandEnvelope::new(1, command_id(2), user(2), 0, command());
    forged.header.chain_hash = original.header.chain_hash;

    let mut replica = Replica::<ChatState>::new();
    assert_eq!(replica.apply_next(&forged), Err(ApplyError::Diverged));
    assert_eq!(replica.version(), 0, "a diverged command must not reach the state");
}

#[test]
fn a_command_with_the_wrong_chain_hash_is_rejected_as_diverged() {
    let mut replica = Replica::<ChatState>::new();
    let mut log = ChatCommandLog::default();
    replica.apply_next(log.add(user(1), "first")).unwrap();

    // The right version, but a chain hash that does not continue this replica's own chain: as if
    // this command, or an earlier one, had been tampered with or reordered before it got here.
    let mut tampered = log.add(user(1), "second").clone();
    tampered.header.chain_hash ^= 1;
    assert_eq!(replica.apply_next(&tampered), Err(ApplyError::Diverged));
    assert_eq!(replica.version(), 1, "a diverged command must not reach the state");
}
