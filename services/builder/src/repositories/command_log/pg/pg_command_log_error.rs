use crate::{models::CommandStreamError, repositories::command_log::StreamHead};
use shine_infra::db::postgres::{PgError, PgPoolError, PgRawError};
use thiserror::Error as ThisError;

/// What can go wrong in the Postgres command log. It is the repository's own error; the
/// `CommandLogDb` implementation turns it into a `CommandStreamError` at the trait boundary, so the
/// model never sees the store it came from.
#[derive(Debug, ThisError)]
pub enum PgCommandLogError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Too much contention appending to the stream")]
    Contention,
    #[error("Drifted from the persisted chain (head at version {})", .head.version)]
    LogDiverged { head: StreamHead },
    #[error("Snapshot drifted from the persisted chain")]
    SnapshotDiverged,
    #[error(transparent)]
    Pg(#[from] PgError),
}

impl From<PgRawError> for PgCommandLogError {
    fn from(err: PgRawError) -> Self {
        Self::Pg(PgError::PgRawError(err))
    }
}

impl From<PgPoolError> for PgCommandLogError {
    fn from(err: PgPoolError) -> Self {
        Self::Pg(PgError::PoolError(err))
    }
}

impl From<PgCommandLogError> for CommandStreamError {
    #[track_caller]
    fn from(err: PgCommandLogError) -> Self {
        match err {
            // Facts about the stream itself: the model acts on each of these.
            PgCommandLogError::StreamNotFound => CommandStreamError::StreamNotFound,
            PgCommandLogError::Contention => CommandStreamError::Contention,
            PgCommandLogError::LogDiverged { head } => CommandStreamError::LogDiverged { head },
            PgCommandLogError::SnapshotDiverged => CommandStreamError::SnapshotDiverged,
            // A pool that cannot hand out a connection is a transient outage, not a bug.
            PgCommandLogError::Pg(err @ (PgError::CreatePoolError(_) | PgError::PoolError(_))) => {
                CommandStreamError::service_unavailable("PostgreSQL", err)
            }
            PgCommandLogError::Pg(err) => CommandStreamError::internal(err),
        }
    }
}
