use crate::db::event_source::{EventSourceError, StreamId};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use uuid::Uuid;

pub trait Event: 'static + Serialize + DeserializeOwned + Send + Sync {
    const NAME: &'static str;

    fn event_type(&self) -> &'static str;
}

/// Stream version with the unique stream token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniqueStreamVersion {
    pub version: usize,
    pub stream_token: Uuid,
}

#[derive(Debug, Clone)]
pub struct StoredEvent<T>
where
    T: Event,
{
    pub version: usize,
    pub stream_token: Uuid,
    pub event: T,
}

pub trait EventStore {
    type Event: Event;
    type StreamId: StreamId;

    /// Creates a new empty stream with version 0. On success, returns a unique stream token
    /// that helps detect ABA issues (create-delete-create scenarios) and ensures events
    /// target the correct stream. The token is not used for stream selection, this is a client-side
    /// feature to detect outdated events, snapshots, or streams.
    /// If the aggregate already exists, the operation fails with a Conflict error.
    fn create_stream(
        &mut self,
        stream_id: &Self::StreamId,
    ) -> impl Future<Output = Result<Uuid, EventSourceError>> + Send;

    /// Get the current version of the given stream.
    fn get_stream_version(
        &mut self,
        stream_id: &Self::StreamId,
    ) -> impl Future<Output = Result<Option<UniqueStreamVersion>, EventSourceError>> + Send;

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
    ) -> impl Future<Output = Result<UniqueStreamVersion, EventSourceError>> + Send;

    /// Get the events in the closed range for the given aggregate.
    fn get_events(
        &mut self,
        stream_id: &Self::StreamId,
        from_version: Option<usize>,
        to_version: Option<usize>,
    ) -> impl Future<Output = Result<Vec<StoredEvent<Self::Event>>, EventSourceError>> + Send;
}
