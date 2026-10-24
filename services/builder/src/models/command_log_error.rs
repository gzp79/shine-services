use crate::repositories::command_log::StreamHead;
use shine_infra::db::postgres::PgError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CommandLogError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Too much contention appending to the stream")]
    Contention,
    #[error("Stream kind mismatch: stored {stored}, requested {requested}")]
    KindMismatch { stored: String, requested: String },
    #[error("Drifted from the persisted chain (head at version {})", .head.version)]
    Diverged { head: StreamHead },
    #[error("Snapshot drifted from the persisted chain")]
    SnapshotDiverged,
    #[error(transparent)]
    Db(#[from] PgError),
}
