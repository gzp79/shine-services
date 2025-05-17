use crate::db::event_source::{EventSourceError, StreamId};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

pub trait Event: Serialize + DeserializeOwned + Send + Sync + 'static {
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
    type StreamId: StreamId;

    /// Create a new empty stream with version 0.
    /// If the aggregate already exists, the operation will fail with a Conflict error.
    fn create_stream(
        &mut self,
        stream_id: &Self::StreamId,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send;

    /// Get the current version of the given stream.
    fn get_stream_version(
        &mut self,
        stream_id: &Self::StreamId,
    ) -> impl Future<Output = Result<Option<usize>, EventSourceError>> + Send;

    /// Delete a stream with all its events and snapshots.    
    fn delete_stream(
        &mut self,
        stream_id: &Self::StreamId,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send;

    /// Store events for an aggregate and return the new version.
    /// This is a checked store operation and will fail if the stream has not ben created or if the expected version is not correct.
    fn store_events(
        &mut self,
        stream_id: &Self::StreamId,
        expected_version: usize,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventSourceError>> + Send;

    /// Store a new event for the given aggregate and return the new version.
    /// This function will create the stream if it does not exist and will store the event with the next available version.
    fn unchecked_store_events(
        &mut self,
        stream_id: &Self::StreamId,
        event: &[Self::Event],
    ) -> impl Future<Output = Result<usize, EventSourceError>> + Send;

    /// Get the events in the closed range for the given aggregate.
    fn get_events(
        &mut self,
        stream_id: &Self::StreamId,
        from_version: Option<usize>,
        to_version: Option<usize>,
    ) -> impl Future<Output = Result<Vec<StoredEvent<Self::Event>>, EventSourceError>> + Send;
}
