use crate::db::event_source::{Event, EventSourceError, StreamId};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

/// Aggregate events into a single model object. Is is sometimes called a "projection".
pub trait Aggregate: 'static + Serialize + DeserializeOwned + Send + Sync {
    type Event: Event;
    type StreamId: StreamId;

    const NAME: &'static str;

    fn apply(&mut self, event: Self::Event) -> Result<(), EventSourceError>;
}

pub struct StoredAggregate<A>
where
    A: Aggregate,
{
    pub stream_id: A::StreamId,
    pub start_version: usize,
    pub version: usize,
    pub aggregate: A,
    pub hash: String,
}

impl<A> StoredAggregate<A>
where
    A: Aggregate,
{
    pub fn from_json(
        stream_id: A::StreamId,
        start_version: usize,
        version: usize,
        data: &str,
        hash: String,
    ) -> Result<Self, EventSourceError> {
        let aggregate = serde_json::from_str(data).map_err(EventSourceError::EventSerialization)?;

        Ok(Self {
            stream_id,
            start_version,
            version,
            aggregate,
            hash,
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AggregateInfo<S>
where
    S: StreamId,
{
    pub stream_id: S,
    pub start_version: usize,
    pub version: usize,
    pub hash: String,
}

/// Store replayed events (snapshots) for a given stream id.
pub trait AggregateStore {
    type Event: Event;
    type StreamId: StreamId;

    /// Store a new snapshot for an aggregate derived from the given snapshot.
    /// A snapshot may have just a single parent and each but first snapshot must have a parent. That is the snapshots can be chained int a single line using the parent version.
    fn store_aggregate<S>(
        &mut self,
        stream_id: &Self::StreamId,
        start_version: usize,
        version: usize,
        aggregate: &S,
        hash: &str,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send
    where
        S: Aggregate<Event = Self::Event, StreamId = Self::StreamId>;

    /// Get the stored aggregate that is not older than the given version
    /// If no version is provided, return the latest snapshot.
    fn get_aggregate<S>(
        &mut self,
        stream_id: &Self::StreamId,
        version: Option<usize>,
    ) -> impl Future<Output = Result<Option<StoredAggregate<S>>, EventSourceError>> + Send
    where
        S: Aggregate<Event = Self::Event, StreamId = Self::StreamId>;

    /// List the available aggregates
    fn list_aggregates<S>(
        &mut self,
        stream_id: &Self::StreamId,
    ) -> impl Future<Output = Result<Vec<AggregateInfo<S::StreamId>>, EventSourceError>> + Send
    where
        S: Aggregate<Event = Self::Event, StreamId = Self::StreamId>,
    {
        self.list_aggregates_by_id(stream_id, S::NAME)
    }

    /// List the available snapshots by stream and aggregate id
    fn list_aggregates_by_id(
        &mut self,
        stream_id: &Self::StreamId,
        aggregate_id: &str,
    ) -> impl Future<Output = Result<Vec<AggregateInfo<Self::StreamId>>, EventSourceError>> + Send;

    /// Delete the snapshots older than the given version.
    fn prune_aggregate<S>(
        &mut self,
        stream_id: &Self::StreamId,
        version: usize,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send
    where
        S: Aggregate<Event = Self::Event, StreamId = Self::StreamId>,
    {
        self.prune_aggregate_by_id(stream_id, S::NAME, version)
    }

    /// Delete the snapshots older than the given version by stream and aggregate id.
    fn prune_aggregate_by_id(
        &mut self,
        stream_id: &Self::StreamId,
        aggregate_id: &str,
        version: usize,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send;
}
