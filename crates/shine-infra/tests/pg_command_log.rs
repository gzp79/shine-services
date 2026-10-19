use shine_infra::db::{
    command_log::{
        decode_snapshot, encode_snapshot, AppendOutcome, CommandLogError, PgCommandLogDb, PruneOutcome,
        SnapshotOutcome, StoredCommand, StreamId,
    },
    postgres::{create_postgres_pool, PgConnectionPool},
};
use shine_test::{db::get_pg_cns, test};
use tokio::sync::OnceCell;
use uuid::Uuid;

static INIT: OnceCell<()> = OnceCell::const_new();

async fn setup() -> Option<PgCommandLogDb> {
    let cns = get_pg_cns()?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let pool: PgConnectionPool = create_postgres_pool(&cns).await.unwrap();
    INIT.get_or_init(|| async {
        let mut client = pool.get().await.unwrap();
        client
            .migrate("command_log", &PgCommandLogDb::migrations())
            .await
            .unwrap();
    })
    .await;
    Some(PgCommandLogDb::new(&pool).await.unwrap())
}

fn cmd(version: i64, generation: i64) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        generation,
        data: format!(r#"{{"type":"add","text":"message {version}"}}"#),
    }
}

fn unique_id() -> StreamId {
    StreamId::parse(&format!("test-{}", Uuid::new_v4())).unwrap()
}

/// Creates a stream and moves it to generation 1, as hydration does.
async fn new_stream(db: &PgCommandLogDb) -> StreamId {
    let stream_id = unique_id();
    db.ensure_stream(&stream_id, "chat").await.unwrap();
    assert_eq!(db.begin_generation(&stream_id).await.unwrap().generation, 1);
    stream_id
}

#[test]
fn stream_ids_are_short_strings_of_safe_characters() {
    assert!(StreamId::parse("chat-global").is_ok());
    assert!(StreamId::parse("a_b.c:d-9").is_ok());
    assert!(StreamId::parse(&"a".repeat(64)).is_ok());
    assert!(StreamId::parse("").is_err());
    assert!(StreamId::parse(&"a".repeat(65)).is_err());
    assert!(StreamId::parse("has space").is_err());
    assert!(StreamId::parse("a/b").is_err());
    assert!(StreamId::parse("ünicode").is_err());
}

#[test]
async fn ensure_stream_creates_once_and_rejects_other_kind() {
    let Some(db) = setup().await else { return };
    let stream_id = unique_id();
    let head = db.ensure_stream(&stream_id, "chat").await.unwrap();
    assert_eq!((head.kind.as_str(), head.version, head.generation), ("chat", 0, 0));
    assert_eq!(db.ensure_stream(&stream_id, "chat").await.unwrap(), head);
    assert!(matches!(
        db.ensure_stream(&stream_id, "other").await,
        Err(CommandLogError::KindMismatch { .. })
    ));
}

#[test]
async fn append_is_idempotent_and_accepts_overlap() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let batch: Vec<_> = (1..=5).map(|v| cmd(v, 1)).collect();

    assert_eq!(
        db.append(&stream_id, &batch[..3]).await.unwrap(),
        AppendOutcome::Appended { version: 3 }
    );
    assert_eq!(
        db.append(&stream_id, &batch[..3]).await.unwrap(),
        AppendOutcome::UpToDate { version: 3 }
    );
    assert_eq!(
        db.append(&stream_id, &batch[1..]).await.unwrap(),
        AppendOutcome::Appended { version: 5 }
    );

    assert_eq!(db.commands_after(&stream_id, 0, 100).await.unwrap(), batch);
    assert_eq!(db.last_commands(&stream_id, 2).await.unwrap(), batch[3..].to_vec());
    assert_eq!(db.head(&stream_id).await.unwrap().unwrap().version, 5);
}

#[test]
async fn append_matches_the_overlap_by_version() {
    // A worker that filled a hole from Postgres appends a list with a gap in it.
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let batch: Vec<_> = (1..=10).map(|v| cmd(v, 1)).collect();
    db.append(&stream_id, &batch).await.unwrap();

    let with_hole = vec![
        batch[1].clone(),
        batch[2].clone(),
        batch[8].clone(),
        batch[9].clone(),
        cmd(11, 1),
    ];
    assert_eq!(
        db.append(&stream_id, &with_hole).await.unwrap(),
        AppendOutcome::Appended { version: 11 }
    );

    let mut other_user = batch[8].clone();
    other_user.user_id = Uuid::new_v4();
    assert_eq!(
        db.append(&stream_id, &[batch[1].clone(), other_user]).await.unwrap(),
        AppendOutcome::Diverged { version: 9 }
    );
}

#[test]
async fn append_rejects_gap_divergence_and_stale_generation() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(&stream_id, &[cmd(1, 1)]).await.unwrap();

    assert_eq!(
        db.append(&stream_id, &[cmd(3, 1)]).await.unwrap(),
        AppendOutcome::Gap { head: 1 }
    );
    assert_eq!(
        db.append(&stream_id, &[cmd(1, 1)]).await.unwrap(),
        AppendOutcome::Diverged { version: 1 },
        "same version, different command id"
    );

    db.begin_generation(&stream_id).await.unwrap(); // generation 2
    assert!(matches!(
        db.append(&stream_id, &[cmd(2, 1)]).await.unwrap(),
        AppendOutcome::StaleGeneration { .. }
    ));
    assert_eq!(
        db.append(&stream_id, &[cmd(2, 2)]).await.unwrap(),
        AppendOutcome::Appended { version: 2 }
    );
}

#[test]
async fn command_text_is_stored_verbatim() {
    // jsonb would reject the \u0000 escape and reformat the text.
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let mut command = cmd(1, 1);
    command.data = r#"{"type":"add","text":"a\u0000b"}"#.to_string();
    db.append(&stream_id, &[command.clone()]).await.unwrap();
    assert_eq!(db.commands_after(&stream_id, 0, 10).await.unwrap(), vec![command]);
}

#[test]
async fn concurrent_identical_appends_converge() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let batch: Vec<_> = (1..=20).map(|v| cmd(v, 1)).collect();

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let (db, batch, stream_id) = (db.clone(), batch.clone(), stream_id.clone());
            tokio::spawn(async move { db.append(&stream_id, &batch).await.unwrap() })
        })
        .collect();
    let mut appended = 0;
    for task in tasks {
        match task.await.unwrap() {
            AppendOutcome::Appended { version: 20 } => appended += 1,
            AppendOutcome::UpToDate { version: 20 } => {}
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(appended, 1);
    assert_eq!(db.commands_after(&stream_id, 0, 100).await.unwrap(), batch);
}

#[test]
async fn snapshot_race_reports_the_stored_hash() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(&stream_id, &(1..=10).map(|v| cmd(v, 1)).collect::<Vec<_>>())
        .await
        .unwrap();

    let hash = u64::MAX - 1; // above i64::MAX, checks the bit cast
    assert_eq!(
        db.try_store_snapshot(&stream_id, 5, hash, b"first").await.unwrap(),
        SnapshotOutcome::Stored
    );
    assert_eq!(
        db.try_store_snapshot(&stream_id, 5, 7, b"second").await.unwrap(),
        SnapshotOutcome::Exists { hash }
    );
    let latest = db.latest_snapshot(&stream_id, 9).await.unwrap().unwrap();
    assert_eq!(
        (latest.version, latest.hash, latest.data.as_slice()),
        (5, hash, &b"first"[..])
    );
    assert!(db.latest_snapshot(&stream_id, 4).await.unwrap().is_none());
}

#[test]
async fn snapshot_below_a_newer_one_is_superseded() {
    // Below the prune horizon a lagging, drifted instance must not certify its own state.
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(&stream_id, &(1..=10).map(|v| cmd(v, 1)).collect::<Vec<_>>())
        .await
        .unwrap();
    assert_eq!(
        db.try_store_snapshot(&stream_id, 10, 1, b"s").await.unwrap(),
        SnapshotOutcome::Stored
    );
    assert_eq!(
        db.try_store_snapshot(&stream_id, 5, 2, b"s").await.unwrap(),
        SnapshotOutcome::Superseded
    );
    assert!(db.latest_snapshot(&stream_id, 9).await.unwrap().is_none());
    assert_eq!(
        db.try_store_snapshot(&stream_id, 10, 3, b"s").await.unwrap(),
        SnapshotOutcome::Exists { hash: 1 }
    );
}

/// 20 commands with snapshots at 5, 10, 15 and 20.
async fn stream_with_snapshots(db: &PgCommandLogDb) -> (StreamId, Vec<StoredCommand>) {
    let stream_id = new_stream(db).await;
    let batch: Vec<_> = (1..=20).map(|v| cmd(v, 1)).collect();
    db.append(&stream_id, &batch).await.unwrap();
    for version in [5, 10, 15, 20] {
        db.try_store_snapshot(&stream_id, version, version as u64, b"s")
            .await
            .unwrap();
    }
    (stream_id, batch)
}

#[test]
async fn prune_keeps_the_latest_snapshots_and_the_commands_from_the_oldest_of_them() {
    let Some(db) = setup().await else { return };
    let (stream_id, batch) = stream_with_snapshots(&db).await;

    assert_eq!(
        db.prune(&stream_id, 2).await.unwrap(),
        PruneOutcome { snapshots: 2, commands: 14 }
    );

    // Snapshots 15 and 20 remain, and so do the commands 15..=20.
    assert_eq!(db.oldest_snapshot_version(&stream_id).await.unwrap(), Some(15));
    assert_eq!(db.latest_snapshot(&stream_id, 19).await.unwrap().unwrap().version, 15);
    assert!(db.latest_snapshot(&stream_id, 14).await.unwrap().is_none());
    assert_eq!(db.latest_snapshot(&stream_id, 20).await.unwrap().unwrap().version, 20);
    assert_eq!(
        db.commands_after(&stream_id, 0, 100).await.unwrap(),
        batch[14..].to_vec()
    );
    assert_eq!(db.head(&stream_id).await.unwrap().unwrap().version, 20);
    assert_eq!(
        db.prune(&stream_id, 2).await.unwrap(),
        PruneOutcome { snapshots: 0, commands: 0 },
        "nothing left to prune"
    );
}

#[test]
async fn prune_with_fewer_snapshots_than_kept_deletes_nothing() {
    let Some(db) = setup().await else { return };
    let (stream_id, _) = stream_with_snapshots(&db).await;
    assert_eq!(
        db.prune(&stream_id, 5).await.unwrap(),
        PruneOutcome { snapshots: 0, commands: 0 }
    );
    assert_eq!(db.commands_after(&stream_id, 0, 100).await.unwrap().len(), 20);
    assert_eq!(db.oldest_snapshot_version(&stream_id).await.unwrap(), Some(5));
}

#[test]
async fn prune_without_a_snapshot_deletes_nothing() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(&stream_id, &(1..=5).map(|v| cmd(v, 1)).collect::<Vec<_>>())
        .await
        .unwrap();
    assert_eq!(
        db.prune(&stream_id, 2).await.unwrap(),
        PruneOutcome { snapshots: 0, commands: 0 }
    );
    assert_eq!(db.commands_after(&stream_id, 0, 100).await.unwrap().len(), 5);
    assert_eq!(db.oldest_snapshot_version(&stream_id).await.unwrap(), None);
}

#[test]
async fn append_still_works_after_pruning() {
    let Some(db) = setup().await else { return };
    let (stream_id, _) = stream_with_snapshots(&db).await;
    db.prune(&stream_id, 2).await.unwrap();

    let next: Vec<_> = (21..=25).map(|v| cmd(v, 1)).collect();
    assert_eq!(
        db.append(&stream_id, &next).await.unwrap(),
        AppendOutcome::Appended { version: 25 }
    );
    assert_eq!(db.last_commands(&stream_id, 5).await.unwrap(), next);
}

#[test]
async fn a_repeated_pruned_row_is_reported_as_diverged() {
    // A stale writer that repeats a row from before the horizon cannot be checked against the log.
    let Some(db) = setup().await else { return };
    let (stream_id, batch) = stream_with_snapshots(&db).await;
    db.prune(&stream_id, 2).await.unwrap();

    assert_eq!(
        db.append(&stream_id, &batch[..3]).await.unwrap(),
        AppendOutcome::Diverged { version: 1 }
    );
}

#[test]
fn a_snapshot_round_trips_and_garbage_is_an_error() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct State {
        messages: Vec<(u128, String)>,
        edited: bool,
    }
    let state = State {
        messages: vec![(1, "hi".into()), (2, "\u{2713}".into())],
        edited: true,
    };
    let bytes = encode_snapshot(&state).unwrap();
    assert_eq!(decode_snapshot::<State>(&bytes).unwrap(), state);
    assert!(decode_snapshot::<State>(&[0xff, 0xff, 0xff]).is_err());
}
