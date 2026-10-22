use crate::{
    models::CommandLogError,
    repositories::command_log::{
        pg::{append, commands, consistency, prune, snapshot, stream},
        CommandLogDb, ConsistencyReport, StoredCommand, StoredSnapshot, StreamHead, StreamId,
    },
};
use shine_infra::db::postgres::{PgConnectionPool, PgError, PgPoolError, PgPooledConnection};
use std::{sync::Arc, time::Duration};

/// Checkout deadline, well under the pool's own `pool_timeout` (30 s by default): a down Postgres
/// fails a request fast instead of parking it for half a minute.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Statements {
    pub ensure_stream: stream::EnsureStream,
    pub get_head: stream::GetHead,
    pub insert_commands: append::InsertCommands,
    pub cas_head_version: append::CasHeadVersion,
    pub commands_after: commands::CommandsAfter,
    pub last_commands: commands::LastCommands,
    pub command_chain_hash_at: snapshot::CommandChainHashAt,
    pub insert_snapshot: snapshot::InsertSnapshot,
    pub get_snapshot: snapshot::GetSnapshot,
    pub latest_snapshot: snapshot::LatestSnapshot,
    pub latest_snapshot_version: snapshot::LatestSnapshotVersion,
    pub oldest_snapshot: snapshot::OldestSnapshot,
    pub prune: prune::Prune,
    pub list_snapshots: consistency::ListSnapshots,
}

struct Inner {
    pool: PgConnectionPool,
    stmts: Statements,
}

#[derive(Clone)]
pub struct PgCommandLogDb(Arc<Inner>);

pub fn db_error(err: impl Into<PgError>) -> CommandLogError {
    CommandLogError::Db(err.into())
}

impl PgCommandLogDb {
    pub async fn new(pool: &PgConnectionPool) -> Result<Self, CommandLogError> {
        let client = pool.get().await.map_err(PgError::PoolError)?;
        let stmts = Statements {
            ensure_stream: stream::EnsureStream::new(&client).await.map_err(db_error)?,
            get_head: stream::GetHead::new(&client).await.map_err(db_error)?,
            insert_commands: append::InsertCommands::new(&client).await.map_err(db_error)?,
            cas_head_version: append::CasHeadVersion::new(&client).await.map_err(db_error)?,
            commands_after: commands::CommandsAfter::new(&client).await.map_err(db_error)?,
            last_commands: commands::LastCommands::new(&client).await.map_err(db_error)?,
            command_chain_hash_at: snapshot::CommandChainHashAt::new(&client).await.map_err(db_error)?,
            insert_snapshot: snapshot::InsertSnapshot::new(&client).await.map_err(db_error)?,
            get_snapshot: snapshot::GetSnapshot::new(&client).await.map_err(db_error)?,
            latest_snapshot: snapshot::LatestSnapshot::new(&client).await.map_err(db_error)?,
            latest_snapshot_version: snapshot::LatestSnapshotVersion::new(&client).await.map_err(db_error)?,
            oldest_snapshot: snapshot::OldestSnapshot::new(&client).await.map_err(db_error)?,
            prune: prune::Prune::new(&client).await.map_err(db_error)?,
            list_snapshots: consistency::ListSnapshots::new(&client).await.map_err(db_error)?,
        };
        Ok(Self(Arc::new(Inner { pool: pool.clone(), stmts })))
    }

    pub fn stmts(&self) -> &Statements {
        &self.0.stmts
    }

    pub async fn client(&self) -> Result<PgPooledConnection<'_>, CommandLogError> {
        match tokio::time::timeout(CLIENT_TIMEOUT, self.0.pool.get()).await {
            Ok(result) => result.map_err(|err| db_error(PgError::PoolError(err))),
            Err(_) => Err(db_error(PgError::PoolError(PgPoolError::TimedOut))),
        }
    }
}

impl CommandLogDb for PgCommandLogDb {
    async fn ensure_stream(&self, stream_id: &StreamId, kind: &str) -> Result<StreamHead, CommandLogError> {
        stream::ensure_stream(self, stream_id, kind).await
    }

    async fn head(&self, stream_id: &StreamId) -> Result<Option<StreamHead>, CommandLogError> {
        stream::head(self, stream_id).await
    }

    async fn append(
        &self,
        stream_id: &StreamId,
        commands: &[StoredCommand],
        baseline_hash: u64,
    ) -> Result<(), CommandLogError> {
        append::append(self, stream_id, commands, baseline_hash).await
    }

    async fn commands_after(
        &self,
        stream_id: &StreamId,
        after: i64,
        limit: i64,
    ) -> Result<Vec<StoredCommand>, CommandLogError> {
        commands::commands_after(self, stream_id, after, limit).await
    }

    async fn last_commands(&self, stream_id: &StreamId, count: i64) -> Result<Vec<StoredCommand>, CommandLogError> {
        commands::last_commands(self, stream_id, count).await
    }

    async fn try_store_snapshot(&self, stream_id: &StreamId, snapshot: &StoredSnapshot) -> Result<(), CommandLogError> {
        snapshot::try_store_snapshot(self, stream_id, snapshot).await
    }

    async fn latest_snapshot(
        &self,
        stream_id: &StreamId,
        at_or_before: i64,
    ) -> Result<Option<StoredSnapshot>, CommandLogError> {
        snapshot::latest_snapshot(self, stream_id, at_or_before).await
    }

    async fn oldest_snapshot_version(&self, stream_id: &StreamId) -> Result<Option<i64>, CommandLogError> {
        snapshot::oldest_snapshot_version(self, stream_id).await
    }

    async fn prune(&self, stream_id: &StreamId, kept_snapshot_count: i64) -> Result<Option<i64>, CommandLogError> {
        prune::prune(self, stream_id, kept_snapshot_count).await
    }

    async fn check_consistency(&self, stream_id: &StreamId) -> Result<ConsistencyReport, CommandLogError> {
        consistency::check_consistency(self, stream_id).await
    }
}
