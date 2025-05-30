use crate::db::event_source::{AggregateStore, Event, EventSourceError, EventStore, StreamId};
use std::future::Future;

pub trait EventDbContext<'c, E, S>:
    EventStore<Event = E, StreamId = S> + AggregateStore<Event = E, StreamId = S> + Send
where
    E: Event,
    S: StreamId,
{
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventNotification<S>
where
    S: StreamId,
{
    StreamCreated {
        stream_id: S,
        version: usize,
    },
    StreamUpdated {
        stream_id: S,
        version: usize,
    },
    StreamDeleted {
        stream_id: S,
    },
    SnapshotCreated {
        stream_id: S,
        aggregate_id: String,
        version: usize,
        hash: String,
    },
    SnapshotDeleted {
        stream_id: S,
        aggregate_id: String,
        version: usize,
    },
}

impl<S> EventNotification<S>
where
    S: StreamId,
{
    pub fn stream_id(&self) -> &S {
        match self {
            EventNotification::StreamCreated { stream_id, .. } => stream_id,
            EventNotification::StreamUpdated { stream_id, .. } => stream_id,
            EventNotification::StreamDeleted { stream_id } => stream_id,
            EventNotification::SnapshotCreated { stream_id, .. } => stream_id,
            EventNotification::SnapshotDeleted { stream_id, .. } => stream_id,
        }
    }
}

pub trait EventDb<E, S>: Send + Sync + 'static
where
    E: Event,
    S: StreamId,
{
    fn create_context(
        &self,
    ) -> impl Future<Output = Result<impl EventDbContext<'_, E, S>, EventSourceError>> + Send;

    fn listen_to_stream_updates<F>(
        &self,
        handler: F,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send
    where
        F: Fn(EventNotification<S>) + Send + Sync + 'static;

    fn unlisten_to_stream_updates(
        &self,
    ) -> impl Future<Output = Result<(), EventSourceError>> + Send;
}
