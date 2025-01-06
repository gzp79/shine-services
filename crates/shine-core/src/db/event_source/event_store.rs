use crate::db::DBError;
use serde::{Deserialize, Serialize};
use std::future::Future;
use thiserror::Error as ThisError;
use uuid::Uuid;

pub trait Event: 'static + Send + Sync + Serialize + for<'de> Deserialize<'de> {
    fn event_type(&self) -> &'static str;
}

pub struct StoredEvent<T>
where
    T: Event,
{
    pub version: usize,
    pub event: T,
}

pub trait Snapshot {
    type Event: Event;

    fn playback(&mut self, aggregate: Uuid, event: &[Self::Event]) -> Result<(), EventStoreError>;
    fn version(&self) -> usize;
}

#[derive(Debug, ThisError)]
pub enum EventStoreError {
    #[error("Conflict")]
    Conflict,
    #[error("Aggregate not found")]
    NotFound,

    #[error(transparent)]
    DbError(#[from] DBError),
}

pub trait EventStore {
    type Event: Event;

    /// Store events for an aggregate and return the new version
    fn store(
        &self,
        aggregate: Uuid,
        expected_version: usize,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventStoreError>> + Send;

    /// Get a range of events for an aggregate
    fn get(
        &self,
        aggregate: Uuid,
        from_version: usize,
        to_version: usize,
    ) -> impl Future<Output = Result<Vec<StoredEvent<Self::Event>>, EventStoreError>> + Send;
}
