// Not called by anything outside this module's own tests yet: the service layer that calls through
// `CommandLogDb` is a later round.
#![allow(dead_code)]

use crate::{
    models::CommandLogError,
    repositories::command_log::{
        CommandLogDb, ConsistencyIssue, ConsistencyReport, StoredCommand, StoredSnapshot, StreamHead, StreamId,
    },
};
use postgres_from_row::FromRow;
use postgres_types::ToSql;
use shine_infra::{
    db::postgres::{PgConnectionPool, PgError, PgPoolError, PgPooledConnection, PgValueTypeUnknown, ToPgType},
    pg_query,
    sync::ExponentialBackoff,
};
use std::{sync::Arc, time::Duration};
use tokio_postgres::{IsolationLevel, Row};
use uuid::Uuid;

/// Bounds of an append transaction: a dead or stuck holder must not keep the stream's row lock.
const APPEND_TIMEOUTS: &str = "SET LOCAL lock_timeout = '5s'; SET LOCAL idle_in_transaction_session_timeout = '30s'";

/// Checkout deadline, well under the pool's own `pool_timeout` (30 s by default): a down Postgres
/// fails a request fast instead of parking it for half a minute.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);

// `chain_hash`/`command_hash`/`hash` are stored as Postgres `BIGINT` (`i64`): Postgres has no
// native unsigned 8-byte type, so the cast to `u64` happens right here, once, at the boundary.
// Hand-written rather than derived so the domain types can be the row types directly. Each row
// type is kept next to the query (or queries) that return it, since none of them are reused
// beyond that.

impl FromRow for StreamHead {
    fn from_row(row: &Row) -> Self {
        Self::try_from_row(row).expect("missing column in row")
    }

    fn try_from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            kind: row.try_get("kind")?,
            version: row.try_get("version")?,
            chain_hash: row.try_get::<_, i64>("chain_hash")? as u64,
        })
    }
}

pg_query!( EnsureStream =>
    in = stream_id: &str, kind: &str;
    out = StreamHead;
    sql = r#"
        INSERT INTO cl_streams (stream_id, kind) VALUES ($1, $2)
        ON CONFLICT (stream_id) DO UPDATE SET stream_id = cl_streams.stream_id
        RETURNING kind, version, chain_hash
    "#
);

pg_query!( GetHead =>
    in = stream_id: &str;
    out = StreamHead;
    sql = "SELECT kind, version, chain_hash FROM cl_streams WHERE stream_id = $1"
);

pg_query!( CasHeadVersion =>
    in = stream_id: &str, expected_version: i64, expected_chain_hash: i64, new_version: i64, new_chain_hash: i64;
    sql = r#"
        UPDATE cl_streams SET version = $4, chain_hash = $5
        WHERE stream_id = $1 AND version = $2 AND chain_hash = $3
    "#
);

/// Row shape of one command bound into `append`'s batch insert; mirrors the `cl_command_input`
/// composite type. Borrows `data` rather than cloning it.
#[derive(Debug, Clone, ToSql)]
#[postgres(name = "cl_command_input")]
struct CommandInput<'a> {
    version: i64,
    command_id: Uuid,
    user_id: Uuid,
    data: &'a str,
    command_hash: i64,
    chain_hash: i64,
}

impl ToPgType for CommandInput<'_> {
    type PgValueType = PgValueTypeUnknown;
}

impl<'a> From<&'a StoredCommand> for CommandInput<'a> {
    fn from(command: &'a StoredCommand) -> Self {
        Self {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            data: command.data.as_str(),
            command_hash: command.command_hash as i64,
            chain_hash: command.chain_hash as i64,
        }
    }
}

// One round trip for the whole batch: the array of commands is unnested into rows instead of one
// execute() per command.
pg_query!( InsertCommands =>
    in = stream_id: &str, commands: &[CommandInput<'_>];
    sql = r#"
        INSERT INTO cl_commands (stream_id, version, command_id, user_id, data, command_hash, chain_hash)
        SELECT $1, (cmd).* FROM UNNEST($2::cl_command_input[]) AS cmd
        ON CONFLICT (stream_id, version) DO NOTHING
    "#
);

impl FromRow for StoredCommand {
    fn from_row(row: &Row) -> Self {
        Self::try_from_row(row).expect("missing column in row")
    }

    fn try_from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            version: row.try_get("version")?,
            command_id: row.try_get("command_id")?,
            user_id: row.try_get("user_id")?,
            data: row.try_get("data")?,
            command_hash: row.try_get::<_, i64>("command_hash")? as u64,
            chain_hash: row.try_get::<_, i64>("chain_hash")? as u64,
        })
    }
}

pg_query!( CommandsAfter =>
    in = stream_id: &str, after: i64, limit: i64;
    out = StoredCommand;
    sql = r#"
        SELECT version, command_id, user_id, data, command_hash, chain_hash FROM cl_commands
        WHERE stream_id = $1 AND version > $2
        ORDER BY version
        LIMIT $3
    "#
);

pg_query!( LastCommands =>
    in = stream_id: &str, count: i64;
    out = StoredCommand;
    sql = r#"
        SELECT * FROM (
            SELECT version, command_id, user_id, data, command_hash, chain_hash FROM cl_commands
            WHERE stream_id = $1
            ORDER BY version DESC
            LIMIT $2
        ) tail ORDER BY version
    "#
);

pg_query!( CommandChainHashAt =>
    in = stream_id: &str, version: i64;
    out = chain_hash: i64;
    sql = "SELECT chain_hash FROM cl_commands WHERE stream_id = $1 AND version = $2"
);

pg_query!( InsertSnapshot =>
    in = stream_id: &str, version: i64, hash: i64, chain_hash: i64, data: Vec<u8>;
    sql = r#"
        INSERT INTO cl_snapshots (stream_id, version, hash, chain_hash, data)
        SELECT $1, $2, $3, $4, $5
        WHERE NOT EXISTS (SELECT 1 FROM cl_snapshots WHERE stream_id = $1 AND version > $2)
        ON CONFLICT (stream_id, version) DO NOTHING
    "#
);

#[derive(FromRow)]
struct SnapshotHashRow {
    version: i64,
    hash: i64,
    chain_hash: i64,
}

pg_query!( GetSnapshot =>
    in = stream_id: &str, version: i64;
    out = SnapshotHashRow;
    sql = "SELECT version, hash, chain_hash FROM cl_snapshots WHERE stream_id = $1 AND version = $2"
);

pg_query!( ListSnapshots =>
    in = stream_id: &str;
    out = SnapshotHashRow;
    sql = "SELECT version, hash, chain_hash FROM cl_snapshots WHERE stream_id = $1 ORDER BY version"
);

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

pg_query!( LatestSnapshot =>
    in = stream_id: &str, at_or_before: i64;
    out = StoredSnapshot;
    sql = r#"
        SELECT version, hash, chain_hash, data FROM cl_snapshots
        WHERE stream_id = $1 AND version <= $2
        ORDER BY version DESC
        LIMIT 1
    "#
);

#[derive(FromRow)]
struct VersionRow {
    version: Option<i64>,
}

pg_query!( OldestSnapshot =>
    in = stream_id: &str;
    out = VersionRow;
    sql = "SELECT MIN(version) AS version FROM cl_snapshots WHERE stream_id = $1"
);

pg_query!( LatestSnapshotVersion =>
    in = stream_id: &str;
    out = VersionRow;
    sql = "SELECT MAX(version) AS version FROM cl_snapshots WHERE stream_id = $1"
);

pg_query!( Prune =>
    in = stream_id: &str, kept_snapshot_count: i64;
    out = version: Option<i64>;
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
        SELECT (SELECT version FROM horizon) AS version
    "#
);

struct Statements {
    ensure_stream: EnsureStream,
    get_head: GetHead,
    cas_head_version: CasHeadVersion,
    insert_commands: InsertCommands,
    commands_after: CommandsAfter,
    last_commands: LastCommands,
    command_chain_hash_at: CommandChainHashAt,
    insert_snapshot: InsertSnapshot,
    get_snapshot: GetSnapshot,
    list_snapshots: ListSnapshots,
    latest_snapshot: LatestSnapshot,
    latest_snapshot_version: LatestSnapshotVersion,
    oldest_snapshot: OldestSnapshot,
    prune: Prune,
}

struct Inner {
    pool: PgConnectionPool,
    stmts: Statements,
}

#[derive(Clone)]
pub struct PgCommandLogDb(Arc<Inner>);

fn db(err: impl Into<PgError>) -> CommandLogError {
    CommandLogError::Db(err.into())
}

impl PgCommandLogDb {
    pub async fn new(pool: &PgConnectionPool) -> Result<Self, CommandLogError> {
        let client = pool.get().await.map_err(PgError::PoolError)?;
        let stmts = Statements {
            ensure_stream: EnsureStream::new(&client).await.map_err(db)?,
            get_head: GetHead::new(&client).await.map_err(db)?,
            cas_head_version: CasHeadVersion::new(&client).await.map_err(db)?,
            insert_commands: InsertCommands::new(&client).await.map_err(db)?,
            commands_after: CommandsAfter::new(&client).await.map_err(db)?,
            last_commands: LastCommands::new(&client).await.map_err(db)?,
            command_chain_hash_at: CommandChainHashAt::new(&client).await.map_err(db)?,
            insert_snapshot: InsertSnapshot::new(&client).await.map_err(db)?,
            get_snapshot: GetSnapshot::new(&client).await.map_err(db)?,
            list_snapshots: ListSnapshots::new(&client).await.map_err(db)?,
            latest_snapshot: LatestSnapshot::new(&client).await.map_err(db)?,
            latest_snapshot_version: LatestSnapshotVersion::new(&client).await.map_err(db)?,
            oldest_snapshot: OldestSnapshot::new(&client).await.map_err(db)?,
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
}

impl CommandLogDb for PgCommandLogDb {
    async fn ensure_stream(&self, stream_id: &StreamId, kind: &str) -> Result<StreamHead, CommandLogError> {
        let client = self.client().await?;
        let id = stream_id.as_str();
        let head: StreamHead = self
            .0
            .stmts
            .ensure_stream
            .query_one(&client, &id, &kind)
            .await
            .map_err(db)?;
        if head.kind != kind {
            return Err(CommandLogError::KindMismatch {
                stored: head.kind,
                requested: kind.to_string(),
            });
        }
        Ok(head)
    }

    async fn head(&self, stream_id: &StreamId) -> Result<Option<StreamHead>, CommandLogError> {
        let client = self.client().await?;
        self.0
            .stmts
            .get_head
            .query_opt(&client, &stream_id.as_str())
            .await
            .map_err(db)
    }

    /// Stores the rows above the head and proves the overlap with it by chain hash.
    async fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
        baseline_hash: u64,
    ) -> Result<(), CommandLogError> {
        debug_assert!(
            commands.windows(2).all(|w| w[1].version == w[0].version + 1),
            "commands must be a dense, contiguous run of versions"
        );
        const MAX_ATTEMPTS: u32 = 8;
        let stmts = &self.0.stmts;
        let id = stream_id.as_str();

        let Some(first) = commands.first() else {
            return Ok(());
        };
        let first_version = first.version;
        let last_version = commands.last().map_or(first_version, |c| c.version);

        let mut backoff = ExponentialBackoff::new(Duration::from_micros(100), Duration::from_millis(100));
        for attempt in 1..=MAX_ATTEMPTS {
            let mut client = self.client().await?;

            // Read and validate outside any transaction: a verify-only (or rejected) append never
            // needs to write at all, and the write below only needs the row for as long as its own
            // statements take.
            let head: StreamHead = stmts
                .get_head
                .query_opt(&client, &id)
                .await
                .map_err(db)?
                .ok_or(CommandLogError::StreamNotFound)?;

            if head.version < first_version - 1 {
                return Err(CommandLogError::Diverged);
            }
            // The chain hash I can claim at head.version, if it falls within what I hold. When the head
            // is already ahead of my own tip, I have nothing of mine to compare yet: deferred to my next
            // append (the snapshot's chain hash is the backstop bounding how long that can go unchecked).
            let my_hash_at_head = if head.version == first_version - 1 {
                Some(baseline_hash)
            } else if head.version <= last_version {
                Some(commands[(head.version - first_version) as usize].chain_hash)
            } else {
                None
            };
            if let Some(expected) = my_hash_at_head {
                if expected != head.chain_hash {
                    return Err(CommandLogError::Diverged);
                }
            }

            // `commands` is a dense, contiguous run from `first_version` (asserted above), so the
            // rows above `head.version` are always a suffix, not an arbitrary subset.
            let new_start = ((head.version - first_version + 1) as usize).min(commands.len());
            let new = &commands[new_start..];
            let Some(new_first) = new.first() else {
                return Ok(());
            };
            if new_first.version != head.version + 1 || new.windows(2).any(|w| w[1].version != w[0].version + 1) {
                return Err(CommandLogError::Diverged);
            }

            let tx = client
                .transaction(Some(IsolationLevel::ReadCommitted))
                .await
                .map_err(db)?;
            tx.batch_execute(APPEND_TIMEOUTS).await.map_err(db)?;

            // Idempotent: a concurrent attempt inserting the same rows at the same versions is harmless
            // (`ON CONFLICT DO NOTHING`); only advancing the head below needs a conflict check. No
            // calculation here: every value is already decided above, outside the transaction. One
            // batch insert, not one round trip per command.
            let commands: Vec<CommandInput<'_>> = new.iter().map(CommandInput::from).collect();
            stmts
                .insert_commands
                .execute(&tx, &id, &commands.as_slice())
                .await
                .map_err(db)?;

            let new_version = new.last().map_or(head.version, |c| c.version);
            let new_chain_hash = new.last().map_or(head.chain_hash, |c| c.chain_hash);
            let moved = stmts
                .cas_head_version
                .execute(
                    &tx,
                    &id,
                    &head.version,
                    &(head.chain_hash as i64),
                    &new_version,
                    &(new_chain_hash as i64),
                )
                .await
                .map_err(db)?;

            if moved == 1 {
                tx.commit().await.map_err(db)?;
                return Ok(());
            }
            tx.rollback().await.map_err(db)?;
            if attempt == MAX_ATTEMPTS {
                return Err(CommandLogError::Contention);
            }
            backoff.delay().await;
        }
        unreachable!()
    }

    async fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> Result<Vec<StoredCommand>, CommandLogError> {
        let client = self.client().await?;
        self.0
            .stmts
            .commands_after
            .query(&client, &stream_id.as_str(), &after, &limit)
            .await
            .map_err(db)
    }

    async fn last_commands(&self, stream_id: &StreamId, count: i64) -> Result<Vec<StoredCommand>, CommandLogError> {
        let client = self.client().await?;
        self.0
            .stmts
            .last_commands
            .query(&client, &stream_id.as_str(), &count)
            .await
            .map_err(db)
    }

    /// Checks the snapshot's `chain_hash` against the command already durably stored at its
    /// version before writing anything: the log, not the caller, is the source of truth.
    async fn try_store_snapshot(&self, stream_id: &StreamId, snapshot: &StoredSnapshot) -> Result<(), CommandLogError> {
        let client = self.client().await?;
        let stmts = &self.0.stmts;
        let id = stream_id.as_str();

        // Never store a snapshot older than the current one: cheap and upfront, before touching the
        // command log at all, since an older snapshot would never be read back anyway.
        let latest_version = stmts
            .latest_snapshot_version
            .query_one(&client, &id)
            .await
            .map_err(db)?
            .version;
        if let Some(latest) = latest_version {
            if snapshot.version < latest {
                return Ok(());
            }
        }

        let logged_chain_hash = stmts
            .command_chain_hash_at
            .query_opt(&client, &id, &snapshot.version)
            .await
            .map_err(db)?;
        match logged_chain_hash {
            Some(chain_hash) if chain_hash as u64 == snapshot.chain_hash => {}
            // Either no command is stored at this version at all, or it disagrees: either way the
            // claim does not match what is durably persisted.
            _ => return Err(CommandLogError::Diverged),
        }

        if let Some(existing) = stmts
            .get_snapshot
            .query_opt(&client, &id, &snapshot.version)
            .await
            .map_err(db)?
        {
            return agrees(&existing, snapshot);
        }

        let inserted = stmts
            .insert_snapshot
            .execute(
                &client,
                &id,
                &snapshot.version,
                &(snapshot.hash as i64),
                &(snapshot.chain_hash as i64),
                &snapshot.data,
            )
            .await
            .map_err(db)?;
        if inserted == 1 {
            return Ok(());
        }
        // Lost the insert race, or a newer snapshot exists (the `WHERE NOT EXISTS` guard blocked it).
        match stmts
            .get_snapshot
            .query_opt(&client, &id, &snapshot.version)
            .await
            .map_err(db)?
        {
            Some(existing) => agrees(&existing, snapshot),
            // A newer snapshot already covers this version; our own claim was already checked above.
            None => Ok(()),
        }
    }

    async fn latest_snapshot(
        &self,
        stream_id: &StreamId,
        at_or_before: i64,
    ) -> Result<Option<StoredSnapshot>, CommandLogError> {
        let client = self.client().await?;
        self.0
            .stmts
            .latest_snapshot
            .query_opt(&client, &stream_id.as_str(), &at_or_before)
            .await
            .map_err(db)
    }

    async fn oldest_snapshot_version(&self, stream_id: &StreamId) -> Result<Option<i64>, CommandLogError> {
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

    /// Keeps the latest `kept_snapshot_count` snapshots and the commands from the oldest of them on,
    /// and deletes the rest in one transaction. Nothing is deleted while the stream has fewer than
    /// `kept_snapshot_count` snapshots. The horizon always comes from the kept snapshots, never from
    /// the caller: a snapshot row is deleted with its command row, so a caller-chosen cutoff could
    /// silently drop snapshots. Pruning never touches `cl_streams`, so the chain hash an append is
    /// checked against is unaffected by what was deleted.
    async fn prune(&self, stream_id: &StreamId, kept_snapshot_count: i64) -> Result<Option<i64>, CommandLogError> {
        let client = self.client().await?;
        self.0
            .stmts
            .prune
            .query_one(&client, &stream_id.as_str(), &kept_snapshot_count.max(1))
            .await
            .map_err(db)
    }

    /// Independently re-verifies the stream from its stored rows, outside any transaction and
    /// without blocking writers: whether the commands form one unbroken chain, every snapshot
    /// agrees with the command at its version, and the head agrees with the last command.
    ///
    /// The head is read first, and every row read after is bounded to that version: a concurrent
    /// append is simply outside this pass rather than a false `HeadMismatch` or `SnapshotDrift`
    /// from a torn read. Reads the whole log up to the head, so this is for tests and operational
    /// checks, not the hot append/snapshot path.
    async fn check_consistency(&self, stream_id: &StreamId) -> Result<ConsistencyReport, CommandLogError> {
        let Some(head) = self.head(stream_id).await? else {
            return Ok(ConsistencyReport {
                checked_up_to_version: 0,
                issues: Vec::new(),
            });
        };

        let mut issues = Vec::new();

        // Ascending order: once a version exceeds the head captured above, every row after it
        // does too, so this is exactly "up to the head", no matter what was appended since.
        let commands: Vec<StoredCommand> = self
            .commands_after(stream_id, 0, i64::MAX)
            .await?
            .into_iter()
            .take_while(|c| c.version <= head.version)
            .collect();

        let mut prev_version = 0i64;
        let mut prev_chain_hash = 0u64;
        for command in &commands {
            if command.version != prev_version + 1 {
                issues.push(ConsistencyIssue::Gap {
                    after: prev_version,
                    found: command.version,
                });
            } else {
                let expected =
                    shine_game::replica::fold_chain_hash(prev_chain_hash, command.version, command.command_hash);
                if expected != command.chain_hash {
                    issues.push(ConsistencyIssue::CommandChainBroken { version: command.version });
                }
            }
            prev_version = command.version;
            prev_chain_hash = command.chain_hash;
        }

        let client = self.client().await?;
        let snapshots = self
            .0
            .stmts
            .list_snapshots
            .query(&client, &stream_id.as_str())
            .await
            .map_err(db)?;
        for snapshot in snapshots {
            if snapshot.version > head.version {
                continue;
            }
            if let Ok(index) = commands.binary_search_by_key(&snapshot.version, |c| c.version) {
                if commands[index].chain_hash != snapshot.chain_hash as u64 {
                    issues.push(ConsistencyIssue::SnapshotDrift { version: snapshot.version });
                }
            }
        }

        let last_chain_hash = commands.last().map_or(0, |c| c.chain_hash);
        if head.chain_hash != last_chain_hash {
            issues.push(ConsistencyIssue::HeadMismatch {
                head_chain_hash: head.chain_hash,
                last_command_chain_hash: last_chain_hash,
            });
        }

        Ok(ConsistencyReport {
            checked_up_to_version: head.version,
            issues,
        })
    }
}

/// `Ok(())` if an already-stored snapshot agrees with what this attempt would have stored.
fn agrees(existing: &SnapshotHashRow, attempted: &StoredSnapshot) -> Result<(), CommandLogError> {
    if existing.hash as u64 == attempted.hash && existing.chain_hash as u64 == attempted.chain_hash {
        Ok(())
    } else {
        Err(CommandLogError::Diverged)
    }
}
