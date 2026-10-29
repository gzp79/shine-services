use crate::repositories::command_log::{
    pg::{pg_command_log_db::PgCommandLogDb, PgCommandLogError},
    StoredSnapshot,
};
use postgres_from_row::FromRow;
use shine_game::replica::StreamId;
use shine_infra::pg_query;
use tokio_postgres::Row;

// `hash`/`chain_hash` are stored as Postgres `BIGINT` (`i64`); the cast to `u64` happens here, at
// the boundary.
impl FromRow for StoredSnapshot {
    fn from_row(row: &Row) -> Self {
        Self::try_from_row(row).expect("missing column in row")
    }

    fn try_from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            version: row.try_get("version")?,
            hash: row.try_get::<_, i64>("hash")? as u64,
            chain_hash: row.try_get::<_, i64>("chain_hash")? as u64,
            data: row.try_get("data")?,
        })
    }
}

#[derive(FromRow)]
struct SnapshotHashRow {
    version: i64,
    hash: i64,
    chain_hash: i64,
}

#[derive(FromRow)]
struct VersionRow {
    version: Option<i64>,
}

pg_query!( pub CommandChainHashAt =>
    in = kind: &str, id: &str, version: i64;
    out = chain_hash: i64;
    sql = "SELECT chain_hash FROM cl_commands WHERE kind = $1 AND id = $2 AND version = $3"
);

pg_query!( pub InsertSnapshot =>
    in = kind: &str, id: &str, version: i64, hash: i64, chain_hash: i64, data: &[u8];
    sql = r#"
        INSERT INTO cl_snapshots (kind, id, version, hash, chain_hash, data)
        SELECT $1, $2, $3, $4, $5, $6
        WHERE NOT EXISTS (SELECT 1 FROM cl_snapshots WHERE kind = $1 AND id = $2 AND version > $3)
        ON CONFLICT (kind, id, version) DO NOTHING
    "#
);

pg_query!( pub GetSnapshot =>
    in = kind: &str, id: &str, version: i64;
    out = SnapshotHashRow;
    sql = "SELECT version, hash, chain_hash FROM cl_snapshots WHERE kind = $1 AND id = $2 AND version = $3"
);

pg_query!( pub LatestSnapshot =>
    in = kind: &str, id: &str, at_or_before: i64;
    out = StoredSnapshot;
    sql = r#"
        SELECT version, hash, chain_hash, data FROM cl_snapshots
        WHERE kind = $1 AND id = $2 AND version <= $3
        ORDER BY version DESC
        LIMIT 1
    "#
);

pg_query!( pub OldestSnapshot =>
    in = kind: &str, id: &str;
    out = VersionRow;
    sql = "SELECT MIN(version) AS version FROM cl_snapshots WHERE kind = $1 AND id = $2"
);

pg_query!( pub LatestSnapshotVersion =>
    in = kind: &str, id: &str;
    out = VersionRow;
    sql = "SELECT MAX(version) AS version FROM cl_snapshots WHERE kind = $1 AND id = $2"
);

/// Checks the snapshot's `chain_hash` against the command already durably stored at its
/// version before writing anything: the log, not the caller, is the source of truth.
pub async fn try_store_snapshot(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    snapshot: &StoredSnapshot,
) -> Result<(), PgCommandLogError> {
    let client = db.client().await?;
    let stmts = db.stmts();
    let kind = stream_id.kind().as_str();
    let id = stream_id.id();

    // Never store a snapshot older than the current one: cheap and upfront, before touching the
    // command log at all, since an older snapshot would never be read back anyway.
    let latest_version = stmts
        .latest_snapshot_version
        .query_one(&client, kind, id)
        .await?
        .version;
    if let Some(latest) = latest_version {
        if snapshot.version < latest {
            return Ok(());
        }
    }

    let logged_chain_hash = stmts
        .command_chain_hash_at
        .query_opt(&client, kind, id, snapshot.version)
        .await?;
    match logged_chain_hash {
        Some(chain_hash) if chain_hash as u64 == snapshot.chain_hash => {}
        // Either no command is stored at this version at all, or it disagrees: either way the
        // claim does not match what is durably persisted.
        _ => return Err(PgCommandLogError::SnapshotDiverged),
    }

    if let Some(existing) = stmts
        .get_snapshot
        .query_opt(&client, kind, id, snapshot.version)
        .await?
    {
        return agrees(&existing, snapshot);
    }

    let inserted = stmts
        .insert_snapshot
        .execute(
            &client,
            kind,
            id,
            snapshot.version,
            snapshot.hash as i64,
            snapshot.chain_hash as i64,
            &snapshot.data,
        )
        .await?;
    if inserted == 1 {
        return Ok(());
    }
    // Lost the insert race, or a newer snapshot exists (the `WHERE NOT EXISTS` guard blocked it).
    match stmts
        .get_snapshot
        .query_opt(&client, kind, id, snapshot.version)
        .await?
    {
        Some(existing) => agrees(&existing, snapshot),
        // A newer snapshot already covers this version; our own claim was already checked above.
        None => Ok(()),
    }
}

pub async fn latest_snapshot(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    at_or_before: i64,
) -> Result<Option<StoredSnapshot>, PgCommandLogError> {
    let client = db.client().await?;
    db.stmts()
        .latest_snapshot
        .query_opt(&client, stream_id.kind().as_str(), stream_id.id(), at_or_before)
        .await
        .map_err(PgCommandLogError::from)
}

pub async fn oldest_snapshot_version(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
) -> Result<Option<i64>, PgCommandLogError> {
    let client = db.client().await?;
    let row = db
        .stmts()
        .oldest_snapshot
        .query_one(&client, stream_id.kind().as_str(), stream_id.id())
        .await?;
    Ok(row.version)
}

/// `Ok(())` if an already-stored snapshot agrees with what this attempt would have stored.
fn agrees(existing: &SnapshotHashRow, attempted: &StoredSnapshot) -> Result<(), PgCommandLogError> {
    if existing.hash as u64 == attempted.hash && existing.chain_hash as u64 == attempted.chain_hash {
        Ok(())
    } else {
        Err(PgCommandLogError::SnapshotDiverged)
    }
}
