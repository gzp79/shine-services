use super::{Fault, Observed, StreamCore, FRAME_COMMANDS};
use crate::{
    models::messages::{StreamMessage, StreamResume, StreamSnapshot},
    repositories::command_stream::redis::{RedisStreamEntry, RedisStreamMeta},
    settings::KindSettings,
};
use shine_game::replay::{encode_snapshot, stable_hash_of, ChatCommand, ChatMessage, ChatState};
use shine_infra::db::command_log::{AppendOutcome, SnapshotOutcome, StoredCommand, StreamHead, StreamId};
use shine_test::test;
use uuid::Uuid;

fn cmd(version: i64, text: &str) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::from_u128(version as u128),
        user_id: Uuid::nil(),
        generation: 1,
        data: serde_json::to_string(&ChatCommand::Add { text: text.to_string() }).unwrap(),
    }
}

fn meta(generation: i64, version: i64, persisted: i64) -> RedisStreamMeta {
    RedisStreamMeta { generation, version, persisted }
}

fn entry(version: i64, command_id: Uuid) -> RedisStreamEntry {
    RedisStreamEntry {
        version,
        generation: 1,
        command_id,
        user_id: Uuid::nil(),
        data: "{}".into(),
    }
}

fn stream_id() -> StreamId {
    StreamId::parse("chat-global").unwrap()
}

/// A loaded core at generation 1 with an empty log, taking a checkpoint every `every` versions.
fn core(every: i64) -> StreamCore<ChatState> {
    let mut core = StreamCore::new(
        stream_id(),
        KindSettings {
            snapshot_every: every,
            keep_snapshots: 3,
        },
    );
    core.begin_load(None);
    core.finish_load(&meta(1, 0, 0));
    core
}

fn apply_redis(core: &mut StreamCore<ChatState>, versions: std::ops::RangeInclusive<i64>) {
    for version in versions {
        core.apply(cmd(version, &format!("m{version}")), false).unwrap();
    }
}

/// Versions delivered to `to`, in frame order.
fn delivered(frames: &[(Uuid, StreamMessage)], to: Uuid) -> Vec<i64> {
    frames
        .iter()
        .filter(|(id, _)| *id == to)
        .flat_map(|(_, frame)| match frame {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                commands.iter().map(|c| c.version).collect()
            }
            _ => vec![],
        })
        .collect()
}

fn texts(core: &StreamCore<ChatState>) -> Vec<String> {
    core.state().messages.iter().map(|m| m.text.clone()).collect()
}

#[test]
fn every_version_is_delivered_once_per_sync_epoch() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = core(100);
    core.add_subscriber(a);
    apply_redis(&mut core, 1..=3);
    let frames = core.take_frames();
    assert!(matches!(frames[0].1, StreamMessage::Sync { .. }));
    assert_eq!(delivered(&frames, a), vec![1, 2, 3]);

    apply_redis(&mut core, 4..=5);
    core.add_subscriber(b);
    let frames = core.take_frames();
    assert_eq!(delivered(&frames, a), vec![4, 5]);
    assert_eq!(delivered(&frames, b), vec![1, 2, 3, 4, 5]);
    assert!(core.take_frames().is_empty(), "nothing new, nothing sent");
}

#[test]
fn large_backlogs_are_split_into_frames() {
    let a = Uuid::new_v4();
    let mut core = core(10_000);
    let total = 2 * FRAME_COMMANDS as i64 + 88;
    apply_redis(&mut core, 1..=total);
    core.add_subscriber(a);
    let frames = core.take_frames();
    let sizes: Vec<_> = frames
        .iter()
        .map(|(_, frame)| match frame {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => commands.len(),
            _ => 0,
        })
        .collect();
    assert_eq!(sizes, vec![FRAME_COMMANDS, FRAME_COMMANDS, 88]);
    assert!(matches!(frames[0].1, StreamMessage::Sync { .. }));
    assert_eq!(delivered(&frames, a), (1..=total).collect::<Vec<_>>());
}

#[test]
fn resolving_a_checkpoint_never_drops_undelivered_commands() {
    let a = Uuid::new_v4();
    let mut core = core(5);
    core.add_subscriber(a);
    core.take_frames();

    // Persist and resolve before fanning out: the commands must still reach the subscriber.
    apply_redis(&mut core, 1..=5);
    core.on_append(AppendOutcome::Appended { version: 5 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(checkpoint.version, 5);
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Stored), None);

    assert!(
        core.take_checkpoint_frames().is_empty(),
        "a checkpoint waits for its commands"
    );
    let frames = core.take_frames();
    assert_eq!(delivered(&frames, a), vec![1, 2, 3, 4, 5]);
    let checkpoints = core.take_checkpoint_frames();
    assert!(matches!(
        checkpoints.as_slice(),
        [(id, StreamMessage::Checkpoint { version: 5, .. })] if *id == a
    ));
}

#[test]
fn reset_is_followed_by_one_sync() {
    let a = Uuid::new_v4();
    let mut core = core(100);
    core.add_subscriber(a);
    apply_redis(&mut core, 1..=2);
    core.take_frames();

    let resets = core.take_reset_frames();
    assert!(matches!(resets.as_slice(), [(id, StreamMessage::Reset { .. })] if *id == a));
    assert!(core.take_reset_frames().is_empty(), "one Reset per Sync");

    core.begin_load(None);
    core.apply(cmd(1, "m1"), true).unwrap();
    core.finish_load(&meta(1, 1, 1));
    let frames = core.take_frames();
    assert!(matches!(frames.as_slice(), [(id, StreamMessage::Sync { .. })] if *id == a));
    assert_eq!(delivered(&frames, a), vec![1]);
}

#[test]
fn a_gap_requires_a_reload() {
    let mut core = core(100);
    assert_eq!(
        core.apply(cmd(2, "late"), false),
        Err(Fault::Reload("gap in the command log"))
    );
}

#[test]
fn a_command_that_cannot_be_decoded_is_a_fault() {
    let mut core = core(100);
    let mut broken = cmd(1, "x");
    broken.data = "not a command".into();
    assert_eq!(core.apply(broken, false), Err(Fault::Undecodable { version: 1 }));
    assert_eq!(core.version(), 0, "the version is not consumed");
}

#[test]
fn observe_adopts_only_proven_newer_generations() {
    let mut core = core(100);
    apply_redis(&mut core, 1..=3);

    assert_eq!(
        core.observe(&meta(0, 3, 0), None),
        Err(Fault::Reset {
            generation: 0,
            reason: "Redis went back to an older generation"
        })
    );
    assert_eq!(
        core.observe(&meta(1, 2, 0), None),
        Err(Fault::Reset {
            generation: 1,
            reason: "Redis is behind the applied commands"
        })
    );
    assert_eq!(core.observe(&meta(2, 3, 3), None), Ok(Observed::NeedsProof(2)));

    let head = StreamHead {
        kind: "chat".into(),
        version: 3,
        generation: 2,
    };
    assert!(core
        .prove_and_adopt(2, AppendOutcome::StaleGeneration { head })
        .is_err());
    core.prove_and_adopt(2, AppendOutcome::UpToDate { version: 3 }).unwrap();
    assert_eq!(core.generation(), 2);
    assert!(core.unpersisted().is_empty());
    assert_eq!(
        core.observe(&meta(2, 3, 3), None),
        Ok(Observed::Current),
        "version 3 is persisted"
    );
}

#[test]
fn observe_detects_a_reassigned_or_lost_version() {
    // After a restart from a dump or a failover, Redis can catch up past the replica with other commands.
    let mut core = core(100);
    apply_redis(&mut core, 1..=3);
    let mine = entry(3, Uuid::from_u128(3));
    let other = entry(3, Uuid::new_v4());
    assert_eq!(core.observe(&meta(1, 5, 0), Some(&mine)), Ok(Observed::Current));
    assert_eq!(
        core.observe(&meta(1, 5, 0), Some(&other)),
        Err(Fault::Reset {
            generation: 1,
            reason: "Redis reassigned an applied version"
        })
    );
    assert_eq!(
        core.observe(&meta(1, 5, 0), None),
        Err(Fault::Reset {
            generation: 1,
            reason: "Redis lost an applied version"
        })
    );
    assert_eq!(
        core.observe(&meta(1, 5, 3), None),
        Ok(Observed::Current),
        "trimmed after it was persisted"
    );
}

#[test]
fn the_persisted_mark_carries_the_command_at_the_version() {
    let mut core = core(100);
    apply_redis(&mut core, 1..=5);
    assert_eq!(core.mark_due(), None, "nothing persisted yet");
    core.on_append(AppendOutcome::Appended { version: 5 }).unwrap();
    assert_eq!(core.mark_due(), Some((5, Uuid::from_u128(5))));
    core.on_marked(5);
    assert_eq!(core.mark_due(), None);
}

#[test]
fn append_outcomes_map_to_faults() {
    let head = StreamHead {
        kind: "chat".into(),
        version: 1,
        generation: 2,
    };
    let cases = [
        (AppendOutcome::Appended { version: 1 }, Ok(())),
        (AppendOutcome::UpToDate { version: 1 }, Ok(())),
        (
            AppendOutcome::StaleGeneration { head },
            Err(Fault::Reset {
                generation: 1,
                reason: "Redis generation differs from Postgres",
            }),
        ),
        (
            AppendOutcome::Gap { head: 0 },
            Err(Fault::Reload("Postgres is behind the applied commands")),
        ),
        (
            AppendOutcome::Diverged { version: 1 },
            Err(Fault::Reset {
                generation: 1,
                reason: "Redis diverged from Postgres",
            }),
        ),
    ];
    for (outcome, expected) in cases {
        let mut core = core(100);
        apply_redis(&mut core, 1..=1);
        assert_eq!(core.on_append(outcome.clone()), expected, "{outcome:?}");
    }
}

#[test]
fn only_the_newest_ready_checkpoint_is_resolved() {
    let mut core = core(1);
    apply_redis(&mut core, 1..=5);
    core.on_append(AppendOutcome::Appended { version: 5 }).unwrap();
    assert_eq!(core.ready_checkpoint().map(|c| c.version), Some(5));
}

#[test]
fn a_superseded_checkpoint_is_dropped() {
    let mut core = core(2);
    apply_redis(&mut core, 1..=2);
    core.on_append(AppendOutcome::Appended { version: 2 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Superseded), None);
    assert!(core.ready_checkpoint().is_none());
    assert!(core.take_checkpoint_frames().is_empty());
}

#[test]
fn rebase_replays_the_newer_commands_on_the_authoritative_state() {
    let mut core = core(2);
    apply_redis(&mut core, 1..=4); // checkpoints captured at 2 and 4
    core.on_append(AppendOutcome::Appended { version: 2 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(checkpoint.version, 2);

    // Another instance stored a different state at version 2.
    let authoritative = ChatState {
        messages: [ChatMessage {
            id: Uuid::from_u128(77),
            user_id: Uuid::nil(),
            text: "authoritative".into(),
            edited: false,
        }]
        .into(),
    };
    let hash = stable_hash_of(&authoritative);
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Exists { hash }), Some(2));

    let snapshot = StreamSnapshot {
        version: 2,
        hash,
        data: encode_snapshot(&authoritative).unwrap().into(),
    };
    core.rebase(snapshot, authoritative).unwrap();
    assert_eq!(core.version(), 4);
    assert_eq!(
        texts(&core),
        vec!["authoritative", "m3", "m4"],
        "versions 3 and 4 replayed on the authoritative state"
    );
}

/// A core at version 6 whose authoritative snapshot is at version 4 (checkpoint every 2, persisted to 4).
fn core_with_a_snapshot_at_4() -> StreamCore<ChatState> {
    let mut core = core(2);
    apply_redis(&mut core, 1..=6);
    core.on_append(AppendOutcome::Appended { version: 4 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(checkpoint.version, 4);
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Stored), None);
    core
}

#[test]
fn resume_inside_the_window_gets_only_the_tail() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = core_with_a_snapshot_at_4();
    core.add_subscriber(a);
    core.take_frames();

    core.add_subscriber_resuming(b, StreamResume { generation: 1, version: 5 });
    let frames = core.take_frames();
    assert!(
        frames
            .iter()
            .filter(|(id, _)| *id == b)
            .all(|(_, frame)| matches!(frame, StreamMessage::Commands { .. })),
        "a resume gets no Sync"
    );
    assert_eq!(delivered(&frames, b), vec![6]);

    // The resumed subscriber follows the live commands like any synced one.
    apply_redis(&mut core, 7..=7);
    assert_eq!(delivered(&core.take_frames(), b), vec![7]);
}

#[test]
fn resume_at_the_head_gets_nothing_and_follows_live() {
    let b = Uuid::new_v4();
    let mut core = core_with_a_snapshot_at_4();
    core.take_frames();
    core.add_subscriber_resuming(b, StreamResume { generation: 1, version: 6 });
    assert!(core.take_frames().is_empty());
    apply_redis(&mut core, 7..=8);
    assert_eq!(delivered(&core.take_frames(), b), vec![7, 8]);
}

#[test]
fn a_resume_that_cannot_be_served_gets_a_full_sync() {
    let b = Uuid::new_v4();
    for (resume, why) in [
        (StreamResume { generation: 9, version: 5 }, "another generation"),
        (
            StreamResume { generation: 1, version: 3 },
            "older than the snapshot of the worker",
        ),
        (StreamResume { generation: 1, version: 7 }, "above the head"),
    ] {
        let mut core = core_with_a_snapshot_at_4();
        core.add_subscriber_resuming(b, resume);
        let frames = core.take_frames();
        assert!(
            matches!(frames.first(), Some((id, StreamMessage::Sync { snapshot: Some(s), .. })) if *id == b && s.version == 4),
            "{why}: {frames:?}"
        );
        assert_eq!(delivered(&frames, b), vec![5, 6], "{why}");
    }
}

#[test]
fn a_reset_turns_a_pending_resume_into_a_full_sync() {
    let b = Uuid::new_v4();
    let mut core = core_with_a_snapshot_at_4();
    core.add_subscriber_resuming(b, StreamResume { generation: 1, version: 5 });
    core.begin_load(None);
    core.apply(cmd(1, "m1"), true).unwrap();
    core.finish_load(&meta(1, 1, 1));
    let frames = core.take_frames();
    assert!(matches!(frames.as_slice(), [(id, StreamMessage::Sync { .. })] if *id == b));
}

#[test]
fn a_subscriber_that_leaves_gets_nothing_more() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = core(100);
    core.add_subscriber(a);
    core.add_subscriber(b);
    core.take_frames();
    core.remove_subscriber(a);
    apply_redis(&mut core, 1..=2);
    let frames = core.take_frames();
    assert!(delivered(&frames, a).is_empty());
    assert_eq!(delivered(&frames, b), vec![1, 2]);
}

#[test]
fn a_sync_announces_the_snapshot_cadence() {
    // A client hashes its state at those versions only, to check the checkpoints of the server.
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = core(7);
    core.add_subscriber(a);
    core.add_subscriber_resuming(b, StreamResume { generation: 9, version: 0 });
    let frames = core.take_frames();
    let cadences: Vec<_> = frames
        .iter()
        .filter_map(|(_, frame)| match frame {
            StreamMessage::Sync { snapshot_every, .. } => Some(*snapshot_every),
            _ => None,
        })
        .collect();
    assert_eq!(cadences, vec![7, 7], "both full syncs carry it");
}
