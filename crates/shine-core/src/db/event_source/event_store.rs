use serde::{Deserialize, Serialize};
use std::future::Future;
use uuid::Uuid;

use crate::db::event_source::EventStoreError;

pub trait Event: 'static + Serialize + for<'de> Deserialize<'de> + Send + Sync {
    const NAME: &'static str;

    fn event_type(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct StoredEvent<T>
where
    T: Event,
{
    pub version: usize,
    pub event: T,
}

pub trait EventStore {
    type Event: Event;

    /// Create a new empty aggregate with version 0.
    /// If the aggregate already exists, the operation will fail with a Conflict error.
    fn create_stream(&mut self, aggregate: &Uuid) -> impl Future<Output = Result<(), EventStoreError>> + Send;

    /// Store events for an aggregate and return the new version
    /// If expected_version is Some, the store will fail if the current version does not match, otherwise it will store the events
    /// emulating a last-write-wins strategy.
    fn store_events(
        &mut self,
        aggregate_id: &Uuid,
        expected_version: Option<usize>,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventStoreError>> + Send;

    /// Get a range of events for an aggregate
    fn get_events(
        &mut self,
        aggregate_id: &Uuid,
        from_version: Option<usize>,
        to_version: Option<usize>,
    ) -> impl Future<Output = Result<Vec<StoredEvent<Self::Event>>, EventStoreError>> + Send;
}
