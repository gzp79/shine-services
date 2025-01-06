use crate::db::DBError;
use serde::{Deserialize, Serialize};
use std::future::Future;
use thiserror::Error as ThisError;
use uuid::Uuid;

pub trait Event: 'static + Serialize + for<'de> Deserialize<'de> + Send + Sync {
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
    EventSerialization(#[from] serde_json::Error),
    #[error(transparent)]
    DbError(#[from] DBError),
}

pub trait EventStore {
    type Event: Event;

    /// Create a new empty aggregate with version 0.
    /// If the aggregate already exists, the operation will fail with a Conflict error.
    fn create(&self, aggregate: &Uuid) -> impl Future<Output = Result<(), EventStoreError>> + Send;

    /// Store events for an aggregate and return the new version
    /// If expected_version is Some, the store will fail if the current version does not match, otherwise it will store the events
    /// emulating a last-write-wins strategy.
    fn store(
        &self,
        aggregate: &Uuid,
        expected_version: Option<usize>,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventStoreError>> + Send;

    /// Get a range of events for an aggregate
    fn get(
        &self,
        aggregate: &Uuid,
        from_version: usize,
        to_version: Option<usize>,
    ) -> impl Future<Output = Result<Vec<StoredEvent<Self::Event>>, EventStoreError>> + Send;
}
