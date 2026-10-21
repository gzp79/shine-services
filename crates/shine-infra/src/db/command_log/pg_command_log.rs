use crate::{
    db::{
        command_log::{
            AppendOutcome, CommandLogError, PruneOutcome, SnapshotOutcome, StoredCommand, StoredSnapshot, StreamHead,
            StreamId,
        },
        postgres::{PgConnection, PgConnectionPool, PgError, PgPoolError, PgPooledConnection, PgRawConnection},
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

const MIGRATION_V1: &str = include_str!("sql/V1__command_log.sql");

/// Bounds of an append transaction: a dead or stuck holder must not keep the stream's row lock.
const APPEND_TIMEOUTS: &str = "SET LOCAL lock_timeout = '5s'; SET LOCAL idle_in_transaction_session_timeout = '30s'";

/// Checkout deadline, well under the pool's own `pool_timeout` (30 s by default): a down Postgres
/// fails a request fast instead of parking it for half a minute.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(FromRow)]
struct HeadRow {
    kind: String,
    version: i64,
    generation: i64,
}

impl From<HeadRow> for StreamHead {
    fn from(row: HeadRow) -> Self {
        Self {
            kind: row.kind,
            version: row.version,
            generation: row.generation,
        }
    }
}

#[derive(FromRow)]
struct CommandRow {
    version: i64,
    command_id: Uuid,
    user_id: Uuid,
    generation: i64,
    data: String,
}

impl From<CommandRow> for StoredCommand {
    fn from(row: CommandRow) -> Self {
        Self {
            version: row.version,
            command_id: row.command_id,
            user_id: row.user_id,
            generation: row.generation,
            data: row.data,
        }
    }
}

#[derive(FromRow)]
struct IdRow {
    version: i64,
    command_id: Uuid,
    user_id: Uuid,
}

#[derive(FromRow)]
struct SnapshotRow {
    version: i64,
    hash: i64,
    data: Vec<u8>,
}

#[derive(FromRow)]
struct PruneRow {
    snapshots: i64,
    commands: i64,
}

#[derive(FromRow)]
struct OldestRow {
    version: Option<i64>,
}

pg_query!( InsertStream =>
    in = stream_id: &str, kind: &str;
    sql = "INSERT INTO cl_streams (stream_id, kind) VALUES ($1, $2) ON CONFLICT (stream_id) DO NOTHING"
);

pg_query!( GetHead =>
    in = stream_id: &str;
    out = HeadRow;
    sql = "SELECT kind, version, generation FROM cl_streams WHERE stream_id = $1"
);

pg_query!( LockHead =>
    in = stream_id: &str;
    out = HeadRow;
    sql = "SELECT kind, version, generation FROM cl_streams WHERE stream_id = $1 FOR UPDATE"
);

pg_query!( BumpGeneration =>
    in = stream_id: &str;
    out = HeadRow;
    sql = r#"
        UPDATE cl_streams SET generation = generation + 1, updated_at = now()
        WHERE stream_id = $1
        RETURNING kind, version, generation
    "#
);

pg_query!( SetHeadVersion =>
    in = stream_id: &str, version: i64;
    sql = "UPDATE cl_streams SET version = $2, updated_at = now() WHERE stream_id = $1"
);

pg_query!( InsertCommand =>
    in = stream_id: &str, version: i64, command_id: Uuid, user_id: Uuid, generation: i64, data: &str;
    sql = r#"
        INSERT INTO cl_commands (stream_id, version, command_id, user_id, generation, data)
        VALUES ($1, $2, $3, $4, $5, $6)
    "#
);

pg_query!( CommandIdsInRange =>
    in = stream_id: &str, from_version: i64, to_version: i64;
    out = IdRow;
    sql = r#"
        SELECT version, command_id, user_id FROM cl_commands
        WHERE stream_id = $1 AND version >= $2 AND version <= $3
    "#
);

pg_query!( CommandsAfter =>
    in = stream_id: &str, after: i64, limit: i64;
    out = CommandRow;
    sql = r#"
        SELECT version, command_id, user_id, generation, data FROM cl_commands
        WHERE stream_id = $1 AND version > $2
        ORDER BY version
        LIMIT $3
    "#
);

pg_query!( LastCommands =>
    in = stream_id: &str, count: i64;
    out = CommandRow;
    sql = r#"
        SELECT * FROM (
            SELECT version, command_id, user_id, generation, data FROM cl_commands
            WHERE stream_id = $1
            ORDER BY version DESC
            LIMIT $2
        ) tail ORDER BY version
    "#
);

pg_query!( InsertSnapshot =>
    in = stream_id: &str, version: i64, hash: i64, data: Vec<u8>;
    sql = r#"
        INSERT INTO cl_snapshots (stream_id, version, hash, data)
        SELECT $1, $2, $3, $4
        WHERE NOT EXISTS (SELECT 1 FROM cl_snapshots WHERE stream_id = $1 AND version > $2)
        ON CONFLICT (stream_id, version) DO NOTHING
    "#
);

pg_query!( GetSnapshotHash =>
    in = stream_id: &str, version: i64;
    out = hash: i64;
    sql = "SELECT hash FROM cl_snapshots WHERE stream_id = $1 AND version = $2"
);

pg_query!( LatestSnapshot =>
    in = stream_id: &str, at_or_before: i64;
    out = SnapshotRow;
    sql = r#"
        SELECT version, hash, data FROM cl_snapshots
        WHERE stream_id = $1 AND version <= $2
        ORDER BY version DESC
        LIMIT 1
    "#
);

pg_query!( OldestSnapshot =>
    in = stream_id: &str;
    out = OldestRow;
    sql = "SELECT MIN(version) AS version FROM cl_snapshots WHERE stream_id = $1"
);

pg_query!( SnapshotHorizon =>
    in = stream_id: &str, keep: i64;
    out = OldestRow;
    sql = r#"
        SELECT (
            SELECT version FROM cl_snapshots WHERE stream_id = $1
            ORDER BY version DESC OFFSET ($2 - 1) LIMIT 1
        ) AS version
    "#
);

// One statement, so one transaction: the oldest kept snapshot defines both what is deleted. A snapshot row
// goes with its command row, so the commands are never cut above the oldest kept snapshot.
pg_query!( Prune =>
    in = stream_id: &str, keep: i64;
    out = PruneRow;
    sql = r#"
        WITH horizon AS (
            SELECT version FROM cl_snapshots WHERE stream_id = $1
            ORDER BY version DESC OFFSET ($2 - 1) LIMIT 1
        ),
        old_snapshots AS (
            DELETE FROM cl_snapshots
            WHERE stream_id = $1 AND version < (SELECT version FROM horizon)
            RETURNING 1
        ),
        old_commands AS (
            DELETE FROM cl_commands
            WHERE stream_id = $1 AND version < (SELECT version FROM horizon)
            RETURNING 1
        )
        SELECT (SELECT count(*) FROM old_snapshots) AS snapshots, (SELECT count(*) FROM old_commands) AS commands
    "#
);

struct Statements {
    insert_stream: InsertStream,
    get_head: GetHead,
    lock_head: LockHead,
    bump_generation: BumpGeneration,
    set_head_version: SetHeadVersion,
    insert_command: InsertCommand,
    command_ids_in_range: CommandIdsInRange,
    commands_after: CommandsAfter,
    last_commands: LastCommands,
    insert_snapshot: InsertSnapshot,
    get_snapshot_hash: GetSnapshotHash,
    latest_snapshot: LatestSnapshot,
    oldest_snapshot: OldestSnapshot,
    snapshot_horizon: SnapshotHorizon,
    prune: Prune,
}

struct Inner {
    pool: PgConnectionPool,
    stmts: Statements,
}

/// Postgres command log. Cheap to clone.
#[derive(Clone)]
pub struct PgCommandLogDb(Arc<Inner>);

fn db(err: impl Into<PgError>) -> CommandLogError {
    CommandLogError::Db(err.into())
}

impl PgCommandLogDb {
    /// Schema migrations; run with `PgConnection::migrate("command_log", ...)`.
    pub fn migrations() -> Vec<String> {
        vec![MIGRATION_V1.to_string()]
    }

    pub async fn new(pool: &PgConnectionPool) -> Result<Self, CommandLogError> {
        let client = pool.get().await.map_err(PgError::PoolError)?;
        let stmts = Statements {
            insert_stream: InsertStream::new(&client).await.map_err(db)?,
            get_head: GetHead::new(&client).await.map_err(db)?,
            lock_head: LockHead::new(&client).await.map_err(db)?,
            bump_generation: BumpGeneration::new(&client).await.map_err(db)?,
            set_head_version: SetHeadVersion::new(&client).await.map_err(db)?,
            insert_command: InsertCommand::new(&client).await.map_err(db)?,
            command_ids_in_range: CommandIdsInRange::new(&client).await.map_err(db)?,
            commands_after: CommandsAfter::new(&client).await.map_err(db)?,
            last_commands: LastCommands::new(&client).await.map_err(db)?,
            insert_snapshot: InsertSnapshot::new(&client).await.map_err(db)?,
            get_snapshot_hash: GetSnapshotHash::new(&client).await.map_err(db)?,
            latest_snapshot: LatestSnapshot::new(&client).await.map_err(db)?,
            oldest_snapshot: OldestSnapshot::new(&client).await.map_err(db)?,
            snapshot_horizon: SnapshotHorizon::new(&client).await.map_err(db)?,
            prune: Prune::new(&client).await.map_err(db)?,
        };
        Ok(Self(Arc::new(Inner { pool: pool.clone(), stmts })))
    }

    async fn client(&self) -> Result<PgPooledConnection<'_>, CommandLogError> {
        match tokio::time::timeout(CLIENT_TIMEOUT, self.0.pool.get()).await {
            Ok(result) => result.map_err(|err| db(PgError::PoolError(err))),
            Err(_) => Err(db(PgError::PoolError(PgPoolError::TimedOut))),
        }
    }

    pub async fn ensure_stream(&self, stream_id: &StreamId, kind: &str) -> Result<StreamHead, CommandLogError> {
        let client = self.client().await?;
        let stmts = &self.0.stmts;
        let id = stream_id.as_str();
        stmts.insert_stream.execute(&client, &id, &kind).await.map_err(db)?;
        let head: StreamHead = stmts.get_head.query_one(&client, &id).await.map_err(db)?.into();
        if head.kind != kind {
            return Err(CommandLogError::KindMismatch {
                stored: head.kind,
                requested: kind.to_string(),
            });
        }
        Ok(head)
    }

    pub async fn head(&self, stream_id: &StreamId) -> Result<Option<StreamHead>, CommandLogError> {
        let client = self.client().await?;
        let head = self
            .0
            .stmts
            .get_head
            .query_opt(&client, &stream_id.as_str())
            .await
            .map_err(db)?;
        Ok(head.map(StreamHead::from))
    }

    /// Starts a new epoch (see `StreamHead::generation`). Serialized with `append` by the row lock.
    pub async fn begin_generation(&self, stream_id: &StreamId) -> Result<StreamHead, CommandLogError> {
        let client = self.client().await?;
        let head = self
            .0
            .stmts
            .bump_generation
            .query_opt(&client, &stream_id.as_str())
            .await
            .map_err(db)?;
        head.map(StreamHead::from).ok_or(CommandLogError::StreamNotFound)
    }

    /// Stores the rows above the head and checks the ones at or below it. `commands` must be ordered
    /// by version; it may have holes below the head.
    pub async fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
    ) -> Result<AppendOutcome, CommandLogError> {
        let mut client = self.client().await?;
        let tx = client
            .transaction(Some(IsolationLevel::ReadCommitted))
            .await
            .map_err(db)?;
        let outcome = self.append_in(&tx, stream_id.as_str(), commands).await;
        let finished = match &outcome {
            Ok(AppendOutcome::Appended { .. }) => tx.commit().await,
            _ => tx.rollback().await,
        };
        // The append's own error matters more than a failed rollback.
        let outcome = outcome?;
        finished.map_err(db)?;
        Ok(outcome)
    }

    async fn append_in<T: PgRawConnection>(
        &self,
        tx: &PgConnection<T>,
        stream_id: &str,
        commands: &[StoredCommand],
    ) -> Result<AppendOutcome, CommandLogError> {
        let stmts = &self.0.stmts;
        tx.batch_execute(APPEND_TIMEOUTS).await.map_err(db)?;
        let head: StreamHead = stmts
            .lock_head
            .query_opt(tx, &stream_id)
            .await
            .map_err(db)?
            .ok_or(CommandLogError::StreamNotFound)?
            .into();

        // Rows at or below the head must be the stored ones, matched by version.
        let overlap: Vec<&StoredCommand> = commands.iter().filter(|c| c.version <= head.version).collect();
        if let (Some(first), Some(last)) = (overlap.first(), overlap.last()) {
            let stored: HashMap<i64, (Uuid, Uuid)> = stmts
                .command_ids_in_range
                .query(tx, &stream_id, &first.version, &last.version)
                .await
                .map_err(db)?
                .into_iter()
                .map(|row| (row.version, (row.command_id, row.user_id)))
                .collect();
            for command in &overlap {
                if stored.get(&command.version) != Some(&(command.command_id, command.user_id)) {
                    return Ok(AppendOutcome::Diverged { version: command.version });
                }
            }
        }

        let new: Vec<&StoredCommand> = commands.iter().filter(|c| c.version > head.version).collect();
        let Some(new_first) = new.first() else {
            return Ok(AppendOutcome::UpToDate { version: head.version });
        };
        if new_first.version != head.version + 1 || new.windows(2).any(|w| w[1].version != w[0].version + 1) {
            return Ok(AppendOutcome::Gap { head: head.version });
        }
        if new.iter().any(|c| c.generation != head.generation) {
            return Ok(AppendOutcome::StaleGeneration { head });
        }

        // Prepared once, then issued together so tokio-postgres pipelines them in one round trip.
        stmts.insert_command.statement(tx).await.map_err(db)?;
        let data: Vec<&str> = new.iter().map(|c| c.data.as_str()).collect();
        futures::future::try_join_all(new.iter().zip(&data).map(|(command, data)| {
            stmts.insert_command.execute(
                tx,
                &stream_id,
                &command.version,
                &command.command_id,
                &command.user_id,
                &command.generation,
                data,
            )
        }))
        .await
        .map_err(db)?;
        let version = new.last().map_or(head.version, |c| c.version);
        stmts
            .set_head_version
            .execute(tx, &stream_id, &version)
            .await
            .map_err(db)?;
        Ok(AppendOutcome::Appended { version })
    }

    pub async fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> Result<Vec<StoredCommand>, CommandLogError> {
        let client = self.client().await?;
        let rows = self
            .0
            .stmts
            .commands_after
            .query(&client, &stream_id.as_str(), &after, &limit)
            .await
            .map_err(db)?;
        Ok(rows.into_iter().map(StoredCommand::from).collect())
    }

    /// The last `count` commands, ordered by version.
    pub async fn last_commands(&self, stream_id: &StreamId, count: i64) -> Result<Vec<StoredCommand>, CommandLogError> {
        let client = self.client().await?;
        let rows = self
            .0
            .stmts
            .last_commands
            .query(&client, &stream_id.as_str(), &count)
            .await
            .map_err(db)?;
        Ok(rows.into_iter().map(StoredCommand::from).collect())
    }

    pub async fn try_store_snapshot(
        &self,
        stream_id: &StreamId,
        version: i64,
        hash: u64,
        data: &[u8],
    ) -> Result<SnapshotOutcome, CommandLogError> {
        let client = self.client().await?;
        let stmts = &self.0.stmts;
        let id = stream_id.as_str();
        // Losers of the race usually find the row here and never upload their bytes.
        if let Some(stored) = stmts
            .get_snapshot_hash
            .query_opt(&client, &id, &version)
            .await
            .map_err(db)?
        {
            return Ok(SnapshotOutcome::Exists { hash: stored as u64 });
        }
        let inserted = stmts
            .insert_snapshot
            .execute(&client, &id, &version, &(hash as i64), &data.to_vec())
            .await
            .map_err(db)?;
        if inserted == 1 {
            return Ok(SnapshotOutcome::Stored);
        }
        // Lost the insert race, or a newer snapshot exists.
        match stmts
            .get_snapshot_hash
            .query_opt(&client, &id, &version)
            .await
            .map_err(db)?
        {
            Some(stored) => Ok(SnapshotOutcome::Exists { hash: stored as u64 }),
            None => Ok(SnapshotOutcome::Superseded),
        }
    }

    pub async fn latest_snapshot(
        &self,
        stream_id: &StreamId,
        at_or_before: i64,
    ) -> Result<Option<StoredSnapshot>, CommandLogError> {
        let client = self.client().await?;
        let row = self
            .0
            .stmts
            .latest_snapshot
            .query_opt(&client, &stream_id.as_str(), &at_or_before)
            .await
            .map_err(db)?;
        Ok(row.map(|row| StoredSnapshot {
            version: row.version,
            hash: row.hash as u64,
            data: row.data,
        }))
    }

    /// The version of the oldest stored snapshot, if there is one.
    pub async fn oldest_snapshot_version(&self, stream_id: &StreamId) -> Result<Option<i64>, CommandLogError> {
        let client = self.client().await?;
        let row = self
            .0
            .stmts
            .oldest_snapshot
            .query_one(&client, &stream_id.as_str())
            .await
            .map_err(db)?;
        Ok(row.version)
    }

    /// The version of the oldest of the latest `keep` snapshots (at least one): the horizon from which the
    /// commands are kept. `None` while the stream has fewer snapshots than that.
    pub async fn snapshot_horizon(&self, stream_id: &StreamId, keep: i64) -> Result<Option<i64>, CommandLogError> {
        let client = self.client().await?;
        let row = self
            .0
            .stmts
            .snapshot_horizon
            .query_one(&client, &stream_id.as_str(), &keep.max(1))
            .await
            .map_err(db)?;
        Ok(row.version)
    }

    /// Keeps the latest `keep` snapshots and the commands from the oldest of them on, and deletes the rest
    /// in one transaction. Nothing is deleted while the stream has fewer than `keep` snapshots. The
    /// horizon always comes from the kept snapshots, never from the caller: a snapshot row is deleted with
    /// its command row, so a caller-chosen cutoff could silently drop snapshots. A later append that
    /// repeats a deleted row is reported as `Diverged`, as it cannot be checked against the log.
    pub async fn prune(&self, stream_id: &StreamId, keep: i64) -> Result<PruneOutcome, CommandLogError> {
        let client = self.client().await?;
        let row = self
            .0
            .stmts
            .prune
            .query_one(&client, &stream_id.as_str(), &keep.max(1))
            .await
            .map_err(db)?;
        Ok(PruneOutcome {
            snapshots: row.snapshots as u64,
            commands: row.commands as u64,
        })
    }
}
