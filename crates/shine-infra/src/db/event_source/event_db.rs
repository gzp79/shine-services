use crate::db::event_source::{AggregateId, Event, EventStore, EventStoreError, SnapshotStore};
use std::future::Future;

pub trait EventDbContext<'c, E, A>:
    EventStore<Event = E, AggregateId = A> + SnapshotStore<Event = E, AggregateId = A> + Send
where
    E: Event,
    A: AggregateId,
{
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventNotification<A>
where
    A: AggregateId,
{
    StreamCreated {
        aggregate_id: A,
        version: usize,
    },
    StreamUpdated {
        aggregate_id: A,
        version: usize,
    },
    StreamDeleted {
        aggregate_id: A,
    },
    SnapshotCreated {
        aggregate_id: A,
        snapshot: String,
        version: usize,
    },
    SnapshotDeleted {
        aggregate_id: A,
        snapshot: String,
        version: usize,
    },
}

impl<A> EventNotification<A>
where
    A: AggregateId,
{
    pub fn aggregate_id(&self) -> &A {
        match self {
            EventNotification::StreamCreated { aggregate_id, .. } => aggregate_id,
            EventNotification::StreamUpdated { aggregate_id, .. } => aggregate_id,
            EventNotification::StreamDeleted { aggregate_id } => aggregate_id,
            EventNotification::SnapshotCreated { aggregate_id, .. } => aggregate_id,
            EventNotification::SnapshotDeleted { aggregate_id, .. } => aggregate_id,
        }
    }
}

pub trait EventDb<E, A>: 'static + Send + Sync
where
    E: Event,
    A: AggregateId,
{
    fn create_context(&self) -> impl Future<Output = Result<impl EventDbContext<'_, E, A>, EventStoreError>> + Send;

    fn listen_to_stream_updates<F>(&self, handler: F) -> impl Future<Output = Result<(), EventStoreError>> + Send
    where
        F: Fn(EventNotification<A>) + Send + Sync + 'static;

    fn unlisten_to_stream_updates(&self) -> impl Future<Output = Result<(), EventStoreError>> + Send;
}
