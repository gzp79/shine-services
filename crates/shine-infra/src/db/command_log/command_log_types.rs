use crate::db::postgres::PgError;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamHead {
    pub kind: String,
    /// Highest persisted version.
    pub version: i64,
    /// History epoch, a fencing token owned by the log: bumped whenever the ordering layer is rebuilt from the log,
    /// so a stale writer of an older epoch is refused and a reader can tell the history after some version was replaced.
    pub generation: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    /// The epoch the command was ordered in; appends must carry the epoch of the head.
    pub generation: i64,
    /// The canonical command JSON.
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub version: i64,
    pub hash: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    /// New rows were stored; `version` is the new head.
    Appended { version: i64 },
    /// Every given row was already stored; `version` is the head.
    UpToDate { version: i64 },
    /// New rows come from another generation than the head's.
    StaleGeneration { head: StreamHead },
    /// The first new row is not `head + 1`, or the new rows are not contiguous.
    Gap { head: i64 },
    /// A given row at or below the head differs from the stored row at `version`.
    Diverged { version: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotOutcome {
    Stored,
    /// A snapshot already exists at the version, with this hash.
    Exists {
        hash: u64,
    },
    /// A newer snapshot exists: the version is below the prune horizon and is not stored.
    Superseded,
}

/// What a prune deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PruneOutcome {
    pub snapshots: u64,
    pub commands: u64,
}

#[derive(Debug, ThisError)]
pub enum CommandLogError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Stream kind mismatch: stored {stored}, requested {requested}")]
    KindMismatch { stored: String, requested: String },
    #[error(transparent)]
    Db(#[from] PgError),
}
