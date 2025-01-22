use crate::db::DBError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum EventStoreError {
    #[error("Conflict")]
    Conflict,
    #[error("Aggregate not found")]
    NotFound,
    #[error("The event to be applied is out of order")]
    EventOutOfOrder,

    #[error(transparent)]
    EventSerialization(#[from] serde_json::Error),
    #[error(transparent)]
    DbError(#[from] DBError),
}
