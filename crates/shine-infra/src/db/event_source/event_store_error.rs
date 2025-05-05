use crate::db::DBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum EventStoreError {
    #[error("Conflict")]
    Conflict,
    #[error("Aggregate not found")]
    AggregateNotFound,
    #[error("The event to be applied is out of order")]
    EventOutOfOrder,
    #[error("Event with the given version {0} not found")]
    EventVersionNotFound(usize),
    #[error("Snapshot with the given version {0} not found")]
    SnapshotVersionNotFound(usize),
    #[error("Snapshot versions is invalid (from {0:?} to {1})")]
    InvalidSnapshotVersion(usize, usize),

    #[error(transparent)]
    EventSerialization(#[from] serde_json::Error),
    #[error(transparent)]
    DbError(#[from] DBError),
}
