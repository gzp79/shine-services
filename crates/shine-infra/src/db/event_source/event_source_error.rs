use crate::db::DBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum EventSourceError {
    #[error("Conflict")]
    Conflict,
    #[error("Stream not found")]
    StreamNotFound,
    #[error("The event to be applied is out of order")]
    EventOutOfOrder,
    #[error("Event with the given version {0} not found")]
    EventVersionNotFound(usize),
    #[error("Snapshot with the given version {0} not found")]
    AggregateVersionNotFound(usize),
    #[error("Snapshot versions is invalid (from {0:?} to {1})")]
    InvalidAggregateVersion(usize, usize),

    #[error(transparent)]
    EventSerialization(#[from] serde_json::Error),
    #[error(transparent)]
    DbError(#[from] DBError),
}
