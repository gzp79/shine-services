use crate::{
    models::CommandLogError,
    repositories::command_log::{
        tests::test_utils::{
            all_commands, append_range, create_test_log, join, replay, seeded_stream, store_room, stream_id,
            stream_with_snapshots, ChatLog, KIND,
        },
        CommandLogDb, ConsistencyIssue, StoredCommand, StoredSnapshot, StreamHead,
    },
};
use shine_test::test;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------------------------
// Happy paths
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_new_stream_is_empty_and_ensuring_it_again_keeps_its_head() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();

    let head = log.db.ensure_stream(&id, KIND).await.unwrap();
    assert_eq!(
        head,
        StreamHead {
            kind: KIND.to_string(),
            version: 0,
            chain_hash: 0
        }
    );
    assert_eq!(log.db.head(&id).await.unwrap(), Some(head));

    let room = ChatLog::new("a", 3);
    append_range(&log.db, &id, &room, 1, 3).await.unwrap();
    let head = log.db.ensure_stream(&id, KIND).await.unwrap();
    assert_eq!((head.version, head.chain_hash), (3, room.chain_hash_at(3)));
}

#[test]
async fn an_append_returns_the_head_the_stream_is_left_at() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let room = ChatLog::new("a", 10);
    let longer = ChatLog::new("a", 14);
    log.db.ensure_stream(&id, KIND).await.unwrap();
    let head_at = |room: &ChatLog, version: i64| StreamHead {
        kind: KIND.to_string(),
        version,
        chain_hash: room.chain_hash_at(version),
    };

    let stored = append_range(&log.db, &id, &room, 1, 10).await.unwrap();
    let covered = append_range(&log.db, &id, &room, 1, 10).await.unwrap();
    let partly_covered = append_range(&log.db, &id, &room, 6, 10).await.unwrap();
    let extended = append_range(&log.db, &id, &longer, 8, 14).await.unwrap();
    let nothing = log.db.append(&id, &[], longer.chain_hash_at(14)).await.unwrap();

    assert_eq!(stored, head_at(&room, 10), "newly stored");
    assert_eq!(covered, head_at(&room, 10), "already covered");
    assert_eq!(partly_covered, head_at(&room, 10), "partly covered");
    assert_eq!(extended, head_at(&longer, 14), "partly covered, then extended");
    assert_eq!(nothing, head_at(&longer, 14), "nothing to append");
}

#[test]
async fn appended_batches_continue_the_chain_and_read_back() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let room = ChatLog::new("a", 50);
    log.db.ensure_stream(&id, KIND).await.unwrap();

    append_range(&log.db, &id, &room, 1, 10).await.unwrap();
    append_range(&log.db, &id, &room, 11, 49).await.unwrap();
    append_range(&log.db, &id, &room, 50, 50).await.unwrap();

    let head = log.db.head(&id).await.unwrap().unwrap();
    assert_eq!((head.version, head.chain_hash), (50, room.chain_hash_at(50)));
    assert_eq!(all_commands(&log.db, &id).await, room.commands);
    assert_eq!(
        log.db.commands_after(&id, 3, 4).await.unwrap(),
        room.commands[3..7],
        "after a version, limited"
    );
    assert_eq!(
        log.db.last_commands(&id, 3).await.unwrap(),
        room.commands[47..],
        "the tail, in ascending order"
    );
}

#[test]
async fn snapshots_are_found_by_the_version_they_cover() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 10, &[3, 6, 9]).await;

    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), Some(3));
    for (at, expected) in [(2, None), (3, Some(3)), (5, Some(3)), (6, Some(6)), (100, Some(9))] {
        let latest = log.db.latest_snapshot(&id, at).await.unwrap();
        assert_eq!(latest.map(|s| s.version), expected, "at or before {at}");
    }
    assert_eq!(log.db.latest_snapshot(&id, 6).await.unwrap(), Some(room.snapshot_at(6)));
}

#[test]
async fn the_stored_log_replays_into_the_room_from_scratch_and_from_a_snapshot() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 40, &[25]).await;
    let head = log.db.head(&id).await.unwrap().unwrap();

    let from_scratch = replay(None, &all_commands(&log.db, &id).await);
    assert_eq!(from_scratch.state(), room.replica().state());
    assert_eq!(from_scratch.chain_hash(), head.chain_hash);

    let snapshot = log.db.latest_snapshot(&id, 40).await.unwrap().unwrap();
    let tail = log.db.commands_after(&id, snapshot.version, i64::MAX).await.unwrap();
    let resumed = replay(Some(&snapshot), &tail);
    assert_eq!(tail.len(), 15);
    assert_eq!(resumed.state(), room.replica().state());
    assert_eq!(resumed.hash(), from_scratch.hash());
    assert_eq!(resumed.chain_hash(), head.chain_hash);
}

#[test]
async fn a_healthy_stream_is_consistent() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = stream_with_snapshots(&log, 10, &[2, 5, 10]).await;

    let report = log.db.check_consistency(&id).await.unwrap();

    assert_eq!(report.checked_up_to_version, 10);
    assert_eq!(report.issues, vec![]);
}

// ---------------------------------------------------------------------------------------------
// Missing and empty streams, kinds
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_missing_stream_reads_as_empty_and_is_never_created_by_reading() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();

    assert_eq!(log.db.head(&id).await.unwrap(), None);
    assert!(log.db.commands_after(&id, 0, 10).await.unwrap().is_empty());
    assert!(log.db.last_commands(&id, 10).await.unwrap().is_empty());
    assert_eq!(log.db.latest_snapshot(&id, 10).await.unwrap(), None);
    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), None);
    assert_eq!(log.db.prune(&id, 1).await.unwrap(), None);
    let report = log.db.check_consistency(&id).await.unwrap();
    assert_eq!((report.checked_up_to_version, report.issues), (0, vec![]));
    assert_eq!(log.db.head(&id).await.unwrap(), None);
}

#[test]
async fn appending_to_a_missing_stream_is_not_found_but_appending_nothing_is_not_an_error() {
    let Some(log) = create_test_log().await else { return };
    let room = ChatLog::new("a", 2);

    let result = append_range(&log.db, &stream_id(), &room, 1, 2).await;
    assert!(matches!(result, Err(CommandLogError::StreamNotFound)), "{result:?}");

    let (id, room) = seeded_stream(&log, 3).await;
    log.db.append(&id, &[], room.chain_hash_at(3)).await.unwrap();
    assert_eq!(log.db.head(&id).await.unwrap().unwrap().version, 3);
}

#[test]
async fn a_stream_cannot_be_ensured_as_another_kind() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    log.db.ensure_stream(&id, KIND).await.unwrap();

    let result = log.db.ensure_stream(&id, "other").await;

    match result {
        Err(CommandLogError::KindMismatch { stored, requested }) => {
            assert_eq!((stored.as_str(), requested.as_str()), (KIND, "other"));
        }
        other => panic!("expected a kind mismatch, got {other:?}"),
    }
}

#[test]
async fn reads_after_the_head_and_of_zero_rows_are_empty() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 4).await;

    assert!(log.db.commands_after(&id, 4, 10).await.unwrap().is_empty());
    assert!(log.db.commands_after(&id, 99, 10).await.unwrap().is_empty());
    assert!(log.db.commands_after(&id, 0, 0).await.unwrap().is_empty());
    assert!(log.db.last_commands(&id, 0).await.unwrap().is_empty());
    assert_eq!(log.db.last_commands(&id, 100).await.unwrap(), room.commands);
}

#[test]
async fn streams_do_not_see_each_other() {
    let Some(log) = create_test_log().await else { return };
    let (first, first_room) = stream_with_snapshots(&log, 12, &[3, 6, 9]).await;
    let second_room = ChatLog::new("other", 7);
    let second = store_room(&log, &second_room).await;

    log.db.prune(&first, 1).await.unwrap();

    assert_eq!(all_commands(&log.db, &second).await, second_room.commands);
    assert_eq!(log.db.oldest_snapshot_version(&second).await.unwrap(), None);
    assert_eq!(all_commands(&log.db, &first).await, first_room.commands[8..]);
    assert_eq!(log.db.head(&second).await.unwrap().unwrap().version, 7);
}

// ---------------------------------------------------------------------------------------------
// Commands that do not belong on the log
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_batch_that_leaves_a_gap_after_the_head_is_diverged() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = seeded_stream(&log, 2).await;
    let longer = ChatLog::new("seed", 6);
    let head = log.db.head(&id).await.unwrap().unwrap();

    let result = append_range(&log.db, &id, &longer, 5, 6).await;

    assert!(
        matches!(&result, Err(CommandLogError::Diverged { head: seen }) if *seen == head),
        "{result:?}"
    );
    assert_eq!(log.db.head(&id).await.unwrap().unwrap(), head);
}

#[test]
async fn a_batch_continuing_from_a_wrong_baseline_is_diverged() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = seeded_stream(&log, 2).await;
    let longer = ChatLog::new("seed", 4);
    let (batch, _) = longer.range(3, 4);
    let head = log.db.head(&id).await.unwrap().unwrap();

    let result = log.db.append(&id, batch, 12345).await;

    assert!(
        matches!(&result, Err(CommandLogError::Diverged { head: seen }) if *seen == head),
        "{result:?}"
    );
    assert_eq!(log.db.head(&id).await.unwrap().unwrap(), head);
}

#[test]
async fn a_different_history_is_diverged_however_it_overlaps_the_head() {
    let Some(log) = create_test_log().await else { return };
    let (id, seed) = seeded_stream(&log, 3).await;
    let other = ChatLog::new("other", 5);
    let head = log.db.head(&id).await.unwrap().unwrap();

    // From the start, ending at the head, and extending the head from a foreign baseline.
    for (first, last) in [(1, 3), (1, 5), (4, 5)] {
        let result = append_range(&log.db, &id, &other, first, last).await;
        assert!(
            matches!(&result, Err(CommandLogError::Diverged { head: seen }) if *seen == head),
            "{first}..={last}: {result:?}"
        );
    }

    assert_eq!(all_commands(&log.db, &id).await, seed.commands);
}

#[test]
async fn a_batch_ending_behind_the_head_is_unverified_and_returns_the_head_beyond_it() {
    let Some(log) = create_test_log().await else { return };
    let (id, seed) = seeded_stream(&log, 5).await;
    let other = ChatLog::new("other", 3);

    // The head is ahead of this batch's tip, so there is nothing of the batch to compare it with:
    // the caller learns it from the returned head being beyond the batch.
    let head = append_range(&log.db, &id, &other, 1, 3).await.unwrap();

    assert_eq!((head.version, head.chain_hash), (5, seed.chain_hash_at(5)));
    assert_eq!(all_commands(&log.db, &id).await, seed.commands);
}

#[test]
async fn resending_covered_commands_is_ok_and_a_partly_covered_batch_stores_only_the_rest() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 5).await;
    let head = log.db.head(&id).await.unwrap().unwrap();

    append_range(&log.db, &id, &room, 1, 5).await.unwrap();
    append_range(&log.db, &id, &room, 3, 5).await.unwrap();
    append_range(&log.db, &id, &room, 5, 5).await.unwrap();
    assert_eq!(log.db.head(&id).await.unwrap().unwrap(), head);
    assert_eq!(all_commands(&log.db, &id).await, room.commands);

    let longer = ChatLog::new("seed", 8);
    append_range(&log.db, &id, &longer, 3, 8).await.unwrap();
    assert_eq!(all_commands(&log.db, &id).await, longer.commands);
    assert_eq!(
        log.db.head(&id).await.unwrap().unwrap().chain_hash,
        longer.chain_hash_at(8)
    );
}

#[test]
async fn a_command_with_a_forged_chain_hash_is_stored_and_found_by_the_consistency_check() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let mut room = ChatLog::new("a", 4);
    room.commands[3].chain_hash ^= 0xdead;
    log.db.ensure_stream(&id, KIND).await.unwrap();

    append_range(&log.db, &id, &room, 1, 4).await.unwrap();

    let report = log.db.check_consistency(&id).await.unwrap();
    assert_eq!(report.issues, vec![ConsistencyIssue::CommandChainBroken { version: 4 }]);
}

#[test]
async fn stored_commands_cannot_be_updated() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 3).await;

    let rejected = log
        .rejects(&format!(
            "UPDATE cl_commands SET data = '{{}}' WHERE stream_id = '{id}' AND version = 2"
        ))
        .await;

    assert!(rejected);
    assert_eq!(all_commands(&log.db, &id).await, room.commands);
}

#[test]
async fn the_inbox_position_of_a_command_is_kept_when_there_is_one_and_is_not_part_of_the_chain() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let mut room = ChatLog::new("a", 4);
    room.commands[2].inbox_id = None;
    log.db.ensure_stream(&id, KIND).await.unwrap();

    append_range(&log.db, &id, &room, 1, 4).await.unwrap();

    assert_eq!(all_commands(&log.db, &id).await, room.commands);
    assert_eq!(log.db.last_commands(&id, 2).await.unwrap(), room.commands[2..]);
    assert_eq!(log.db.check_consistency(&id).await.unwrap().issues, vec![]);
}

#[test]
async fn hashes_above_i64_max_survive_the_signed_column() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 3).await;
    let command = StoredCommand {
        version: 4,
        command_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        data: "{}".to_string(),
        command_hash: u64::MAX,
        chain_hash: u64::MAX - 1,
        inbox_id: None,
    };
    log.db
        .append(&id, std::slice::from_ref(&command), room.chain_hash_at(3))
        .await
        .unwrap();
    // Not the snapshot of the room, only the widest values the columns have to hold.
    let snapshot = StoredSnapshot {
        version: 4,
        hash: u64::MAX,
        chain_hash: u64::MAX - 1,
        data: vec![1],
    };

    log.db.try_store_snapshot(&id, &snapshot).await.unwrap();

    assert_eq!(log.db.head(&id).await.unwrap().unwrap().chain_hash, u64::MAX - 1);
    assert_eq!(log.db.last_commands(&id, 1).await.unwrap(), vec![command]);
    assert_eq!(log.db.latest_snapshot(&id, 4).await.unwrap(), Some(snapshot));
}

// ---------------------------------------------------------------------------------------------
// Snapshots that do not belong on the log
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_snapshot_not_matching_the_command_at_its_version_is_diverged_and_not_stored() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 5).await;
    let forged = StoredSnapshot {
        chain_hash: room.chain_hash_at(5) ^ 1,
        ..room.snapshot_at(5)
    };
    let beyond = StoredSnapshot {
        version: 6,
        ..room.snapshot_at(5)
    };

    for snapshot in [forged, beyond] {
        let result = log.db.try_store_snapshot(&id, &snapshot).await;
        assert!(
            matches!(result, Err(CommandLogError::SnapshotDiverged)),
            "version {}: {result:?}",
            snapshot.version
        );
    }

    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), None);
}

#[test]
async fn a_snapshot_of_a_stream_without_commands_is_diverged() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    log.db.ensure_stream(&id, KIND).await.unwrap();
    let room = ChatLog::new("a", 1);

    let result = log.db.try_store_snapshot(&id, &room.snapshot_at(1)).await;

    assert!(matches!(result, Err(CommandLogError::SnapshotDiverged)), "{result:?}");
}

#[test]
async fn a_snapshot_older_than_the_newest_is_ignored() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 10, &[6]).await;

    log.db.try_store_snapshot(&id, &room.snapshot_at(3)).await.unwrap();

    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), Some(6));
    assert_eq!(log.db.latest_snapshot(&id, 5).await.unwrap(), None);
}

#[test]
async fn a_snapshot_newer_than_the_newest_is_stored_next_to_it() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 10, &[3]).await;

    log.db.try_store_snapshot(&id, &room.snapshot_at(6)).await.unwrap();

    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), Some(3));
    assert_eq!(log.db.latest_snapshot(&id, 10).await.unwrap().unwrap().version, 6);
}

#[test]
async fn storing_a_snapshot_twice_is_ok_but_a_disagreeing_one_at_the_same_version_is_diverged() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 5, &[5]).await;
    let stored = room.snapshot_at(5);
    let disagreeing = StoredSnapshot {
        hash: stored.hash ^ 1,
        ..stored.clone()
    };

    log.db.try_store_snapshot(&id, &stored).await.unwrap();
    let result = log.db.try_store_snapshot(&id, &disagreeing).await;

    assert!(matches!(result, Err(CommandLogError::SnapshotDiverged)), "{result:?}");
    assert_eq!(log.db.latest_snapshot(&id, 5).await.unwrap(), Some(stored));
}

#[test]
async fn snapshot_data_round_trips_byte_for_byte() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 4).await;
    let small = StoredSnapshot {
        data: vec![0, 255, 128, 0, 1],
        ..room.snapshot_at(2)
    };
    let empty = StoredSnapshot {
        data: Vec::new(),
        ..room.snapshot_at(3)
    };
    let large = StoredSnapshot {
        data: (0..200_000u32).map(|i| (i % 251) as u8).collect(),
        ..room.snapshot_at(4)
    };

    for stored in [&small, &empty, &large] {
        log.db.try_store_snapshot(&id, stored).await.unwrap();
    }

    assert_eq!(log.db.latest_snapshot(&id, 2).await.unwrap(), Some(small));
    assert_eq!(log.db.latest_snapshot(&id, 3).await.unwrap(), Some(empty));
    assert_eq!(log.db.latest_snapshot(&id, 4).await.unwrap(), Some(large));
}

// ---------------------------------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------------------------------

#[test]
async fn concurrent_identical_appends_all_succeed_and_store_once() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let room = Arc::new(ChatLog::new("a", 20));
    log.db.ensure_stream(&id, KIND).await.unwrap();

    let appends = (0..8)
        .map(|_| {
            let (db, id, room) = (log.db.clone(), id.clone(), room.clone());
            tokio::spawn(async move { append_range(&db, &id, &room, 1, 20).await })
        })
        .collect();
    for result in join(appends).await {
        result.unwrap();
    }

    assert_eq!(log.db.head(&id).await.unwrap().unwrap().version, 20);
    assert_eq!(all_commands(&log.db, &id).await, room.commands);
}

#[test]
async fn concurrent_appends_of_different_lengths_of_one_history_converge_on_the_longest() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let room = Arc::new(ChatLog::new("a", 30));
    log.db.ensure_stream(&id, KIND).await.unwrap();

    let appends = [5, 30, 12, 21, 30, 8]
        .into_iter()
        .map(|last| {
            let (db, id, room) = (log.db.clone(), id.clone(), room.clone());
            tokio::spawn(async move { append_range(&db, &id, &room, 1, last).await })
        })
        .collect();
    for result in join(appends).await {
        result.unwrap();
    }

    let head = log.db.head(&id).await.unwrap().unwrap();
    assert_eq!((head.version, head.chain_hash), (30, room.chain_hash_at(30)));
    assert_eq!(all_commands(&log.db, &id).await, room.commands);
    assert_eq!(log.db.check_consistency(&id).await.unwrap().issues, vec![]);
}

#[test]
async fn of_concurrent_writers_of_diverging_histories_exactly_one_wins() {
    let Some(log) = create_test_log().await else { return };

    for round in 0..5 {
        let id = stream_id();
        let rooms = [Arc::new(ChatLog::new("a", 10)), Arc::new(ChatLog::new("b", 10))];
        log.db.ensure_stream(&id, KIND).await.unwrap();

        let writers = rooms
            .iter()
            .map(|room| {
                let (db, id, room) = (log.db.clone(), id.clone(), room.clone());
                tokio::spawn(async move { append_range(&db, &id, &room, 1, 10).await })
            })
            .collect();
        let results = join(writers).await;

        let winners: Vec<_> = results.iter().enumerate().filter(|(_, r)| r.is_ok()).collect();
        assert_eq!(winners.len(), 1, "round {round}: {results:?}");
        assert!(
            results
                .iter()
                .any(|r| matches!(r, Err(CommandLogError::Diverged { .. }))),
            "round {round}: {results:?}"
        );
        let winner = &rooms[winners[0].0];
        assert_eq!(all_commands(&log.db, &id).await, winner.commands, "round {round}");
        let head = log.db.head(&id).await.unwrap().unwrap();
        assert_eq!(head.chain_hash, winner.chain_hash_at(10), "round {round}");
        assert_eq!(
            log.db.check_consistency(&id).await.unwrap().issues,
            vec![],
            "round {round}"
        );
    }
}

#[test]
async fn concurrently_ensuring_a_new_stream_creates_it_once() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();

    let ensures = (0..8)
        .map(|_| {
            let (db, id) = (log.db.clone(), id.clone());
            tokio::spawn(async move { db.ensure_stream(&id, KIND).await })
        })
        .collect();
    for result in join(ensures).await {
        assert_eq!(result.unwrap().version, 0);
    }
}

#[test]
async fn of_concurrent_ensures_with_different_kinds_the_first_kind_wins() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();

    let ensures = (0..8)
        .map(|i| {
            let (db, id) = (log.db.clone(), id.clone());
            let kind = if i % 2 == 0 { "kind-a" } else { "kind-b" };
            tokio::spawn(async move { (kind, db.ensure_stream(&id, kind).await) })
        })
        .collect();
    let results = join(ensures).await;

    let stored = log.db.head(&id).await.unwrap().unwrap().kind;
    for (kind, result) in results {
        match result {
            Ok(head) => assert_eq!((kind, head.kind.as_str()), (stored.as_str(), stored.as_str())),
            Err(CommandLogError::KindMismatch { stored: seen, requested }) => {
                assert_eq!((seen, requested.as_str()), (stored.clone(), kind));
                assert_ne!(kind, stored);
            }
            Err(err) => panic!("{kind}: {err:?}"),
        }
    }
}

#[test]
async fn concurrent_identical_snapshots_all_succeed() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 5).await;
    let stored = room.snapshot_at(5);

    let stores = (0..8)
        .map(|_| {
            let (db, id, stored) = (log.db.clone(), id.clone(), stored.clone());
            tokio::spawn(async move { db.try_store_snapshot(&id, &stored).await })
        })
        .collect();
    for result in join(stores).await {
        result.unwrap();
    }

    assert_eq!(log.db.latest_snapshot(&id, 5).await.unwrap(), Some(stored));
}

#[test]
async fn of_concurrent_snapshots_the_newest_is_the_latest_and_the_log_stays_consistent() {
    let Some(log) = create_test_log().await else { return };

    for round in 0..5 {
        let (id, room) = seeded_stream(&log, 10).await;
        let room = Arc::new(room);

        let stores = [4, 8, 2, 6, 10]
            .into_iter()
            .map(|version| {
                let (db, id, room) = (log.db.clone(), id.clone(), room.clone());
                tokio::spawn(async move { db.try_store_snapshot(&id, &room.snapshot_at(version)).await })
            })
            .collect();
        for result in join(stores).await {
            result.unwrap();
        }

        let latest = log.db.latest_snapshot(&id, i64::MAX).await.unwrap();
        assert_eq!(latest, Some(room.snapshot_at(10)), "round {round}");
        assert_eq!(
            log.db.check_consistency(&id).await.unwrap().issues,
            vec![],
            "round {round}"
        );
    }
}

#[test]
async fn appending_while_pruning_loses_nothing_after_the_horizon() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = stream_with_snapshots(&log, 12, &[3, 6, 9]).await;
    let longer = Arc::new(ChatLog::new("seed", 30));

    let appender = {
        let (db, id, room) = (log.db.clone(), id.clone(), longer.clone());
        tokio::spawn(async move {
            for first in (13..=30).step_by(3) {
                append_range(&db, &id, &room, first, first + 2).await.unwrap();
            }
        })
    };
    let pruner = {
        let (db, id) = (log.db.clone(), id.clone());
        tokio::spawn(async move {
            for _ in 0..10 {
                db.prune(&id, 1).await.unwrap();
                tokio::task::yield_now().await;
            }
        })
    };
    join(vec![appender, pruner]).await;

    let versions: Vec<_> = all_commands(&log.db, &id).await.iter().map(|c| c.version).collect();
    assert_eq!(versions, (9..=30).collect::<Vec<_>>());
    assert_eq!(log.db.head(&id).await.unwrap().unwrap().version, 30);
    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), Some(9));
    let snapshot = log.db.latest_snapshot(&id, 30).await.unwrap().unwrap();
    let tail = log.db.commands_after(&id, snapshot.version, i64::MAX).await.unwrap();
    assert_eq!(replay(Some(&snapshot), &tail).state(), longer.replica().state());
}

#[test]
async fn a_consistency_check_during_appends_never_sees_a_torn_stream() {
    let Some(log) = create_test_log().await else { return };
    let id = stream_id();
    let room = Arc::new(ChatLog::new("a", 200));
    log.db.ensure_stream(&id, KIND).await.unwrap();

    let writer = {
        let (db, id, room) = (log.db.clone(), id.clone(), room.clone());
        tokio::spawn(async move {
            for first in (1..=200).step_by(5) {
                append_range(&db, &id, &room, first, first + 4).await.unwrap();
            }
        })
    };
    while !writer.is_finished() {
        let report = log.db.check_consistency(&id).await.unwrap();
        assert_eq!(report.issues, vec![], "checked up to {}", report.checked_up_to_version);
    }
    writer.await.unwrap();

    let report = log.db.check_consistency(&id).await.unwrap();
    assert_eq!((report.checked_up_to_version, report.issues), (200, vec![]));
}

// ---------------------------------------------------------------------------------------------
// Pruning
// ---------------------------------------------------------------------------------------------

#[test]
async fn prune_keeps_the_latest_snapshots_and_the_commands_from_the_oldest_of_them() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 12, &[3, 6, 9]).await;

    let horizon = log.db.prune(&id, 2).await.unwrap();

    assert_eq!(horizon, Some(6));
    assert_eq!(all_commands(&log.db, &id).await, room.commands[5..]);
    assert_eq!(log.db.oldest_snapshot_version(&id).await.unwrap(), Some(6));
    assert_eq!(log.db.latest_snapshot(&id, 5).await.unwrap(), None);
    assert_eq!(log.db.prune(&id, 2).await.unwrap(), Some(6), "idempotent");
    assert_eq!(all_commands(&log.db, &id).await, room.commands[5..]);
}

#[test]
async fn prune_deletes_nothing_without_enough_snapshots_and_never_drops_the_last_one() {
    let Some(log) = create_test_log().await else { return };
    let (few, few_room) = stream_with_snapshots(&log, 12, &[3, 6]).await;
    let (none, none_room) = seeded_stream(&log, 5).await;
    let (zero, zero_room) = stream_with_snapshots(&log, 12, &[3, 6, 9]).await;

    assert_eq!(log.db.prune(&few, 3).await.unwrap(), None);
    assert_eq!(all_commands(&log.db, &few).await, few_room.commands);
    assert_eq!(log.db.prune(&none, 1).await.unwrap(), None);
    assert_eq!(all_commands(&log.db, &none).await, none_room.commands);

    assert_eq!(log.db.prune(&zero, 0).await.unwrap(), Some(9), "0 is read as 1");
    assert_eq!(all_commands(&log.db, &zero).await, zero_room.commands[8..]);
    assert_eq!(log.db.oldest_snapshot_version(&zero).await.unwrap(), Some(9));
}

#[test]
async fn a_pruned_stream_keeps_its_head_and_stays_appendable() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = stream_with_snapshots(&log, 12, &[3, 6, 9]).await;
    let head = log.db.head(&id).await.unwrap().unwrap();
    let longer = ChatLog::new("seed", 15);

    log.db.prune(&id, 1).await.unwrap();

    assert_eq!(log.db.head(&id).await.unwrap().unwrap(), head);
    append_range(&log.db, &id, &longer, 13, 15).await.unwrap();
    assert_eq!(log.db.head(&id).await.unwrap().unwrap().version, 15);
    assert_eq!(all_commands(&log.db, &id).await, longer.commands[8..]);
}

#[test]
async fn a_pruned_stream_replays_from_its_oldest_snapshot() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 40, &[10, 20, 30]).await;

    log.db.prune(&id, 2).await.unwrap();

    let oldest = log.db.oldest_snapshot_version(&id).await.unwrap().unwrap();
    let snapshot = log.db.latest_snapshot(&id, oldest).await.unwrap().unwrap();
    let replica = replay(Some(&snapshot), &all_commands(&log.db, &id).await);
    assert_eq!(oldest, 20);
    assert_eq!(replica.state(), room.replica().state());
    assert_eq!(replica.chain_hash(), room.replica().chain_hash());
}

#[test]
async fn a_pruned_stream_is_consistent() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = stream_with_snapshots(&log, 12, &[3, 6, 9]).await;
    log.db.prune(&id, 2).await.unwrap();

    let report = log.db.check_consistency(&id).await.unwrap();

    assert_eq!(report.checked_up_to_version, 12);
    assert_eq!(report.issues, vec![]);
}

// ---------------------------------------------------------------------------------------------
// The consistency check finds damage
// ---------------------------------------------------------------------------------------------

#[test]
async fn a_missing_command_is_a_gap() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = seeded_stream(&log, 6).await;
    log.exec(&format!(
        "DELETE FROM cl_commands WHERE stream_id = '{id}' AND version = 4"
    ))
    .await;

    let report = log.db.check_consistency(&id).await.unwrap();

    assert_eq!(report.checked_up_to_version, 6);
    assert_eq!(report.issues, vec![ConsistencyIssue::Gap { after: 3, found: 5 }]);
}

#[test]
async fn a_snapshot_off_the_log_is_drift() {
    let Some(log) = create_test_log().await else { return };
    let (id, _) = stream_with_snapshots(&log, 6, &[5]).await;
    log.exec(&format!(
        "UPDATE cl_snapshots SET chain_hash = chain_hash # 1 WHERE stream_id = '{id}'"
    ))
    .await;

    let report = log.db.check_consistency(&id).await.unwrap();

    assert_eq!(report.issues, vec![ConsistencyIssue::SnapshotDrift { version: 5 }]);
}

#[test]
async fn a_head_off_the_last_command_is_a_mismatch() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = seeded_stream(&log, 6).await;
    log.exec(&format!(
        "UPDATE cl_streams SET chain_hash = chain_hash # 1 WHERE stream_id = '{id}'"
    ))
    .await;

    let report = log.db.check_consistency(&id).await.unwrap();

    assert_eq!(
        report.issues,
        vec![ConsistencyIssue::HeadMismatch {
            head_chain_hash: room.chain_hash_at(6) ^ 1,
            last_command_chain_hash: room.chain_hash_at(6),
        }]
    );
}

#[test]
async fn every_issue_is_reported_in_a_single_pass() {
    let Some(log) = create_test_log().await else { return };
    let (id, room) = stream_with_snapshots(&log, 8, &[2]).await;
    log.exec(&format!(
        "DELETE FROM cl_commands WHERE stream_id = '{id}' AND version = 5;
         UPDATE cl_snapshots SET chain_hash = chain_hash # 1 WHERE stream_id = '{id}';
         UPDATE cl_streams SET chain_hash = chain_hash # 1 WHERE stream_id = '{id}';"
    ))
    .await;

    let report = log.db.check_consistency(&id).await.unwrap();

    assert_eq!(
        report.issues,
        vec![
            ConsistencyIssue::Gap { after: 4, found: 6 },
            ConsistencyIssue::SnapshotDrift { version: 2 },
            ConsistencyIssue::HeadMismatch {
                head_chain_hash: room.chain_hash_at(8) ^ 1,
                last_command_chain_hash: room.chain_hash_at(8),
            },
        ]
    );
}
