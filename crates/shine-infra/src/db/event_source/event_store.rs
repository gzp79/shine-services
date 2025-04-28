use crate::db::event_source::{AggregateId, EventStoreError};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

pub trait Event: 'static + Serialize + DeserializeOwned + Send + Sync {
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
    type AggregateId: AggregateId;

    /// Create a new empty stream with version 0.
    /// If the aggregate already exists, the operation will fail with a Conflict error.
    fn create_stream(
        &mut self,
        aggregate: &Self::AggregateId,
    ) -> impl Future<Output = Result<(), EventStoreError>> + Send;

    /// Return if a stream exists
    fn has_stream(
        &mut self,
        aggregate: &Self::AggregateId,
    ) -> impl Future<Output = Result<bool, EventStoreError>> + Send;

    /// Delete a stream with all its events and snapshots.    
    fn delete_stream(
        &mut self,
        aggregate: &Self::AggregateId,
    ) -> impl Future<Output = Result<(), EventStoreError>> + Send;

    /// Store events for an aggregate and return the new version.
    /// This is a checked store operation and will fail if the stream has not ben created or if the expected version is not correct.
    fn store_events(
        &mut self,
        aggregate_id: &Self::AggregateId,
        expected_version: usize,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventStoreError>> + Send;

    /// Store a new event for the given aggregate and return the new version.
    /// This function will create the stream if it does not exist and will store the event with the next available version.
    fn unchecked_store_events(
        &mut self,
        aggregate_id: &Self::AggregateId,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventStoreError>> + Send;

    /// Get a range of events for an aggregate
    fn get_events(
        &mut self,
        aggregate_id: &Self::AggregateId,
        from_version: Option<usize>,
        to_version: Option<usize>,
    ) -> impl Future<Output = Result<Vec<StoredEvent<Self::Event>>, EventStoreError>> + Send;
}
