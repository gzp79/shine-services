use crate::db::event_source::{AggregateId, Event, EventStore, EventStoreError, SnapshotStore};
use std::future::Future;

pub trait EventDbContext<'c, E, A>:
    EventStore<Event = E, AggregateId = A> + SnapshotStore<Event = E, AggregateId = A> + Send
where
    E: Event,
    A: AggregateId,
{
}

#[derive(Debug, Clone)]
pub enum EventNotification<A>
where
    A: AggregateId,
{
    Insert { aggregate_id: A },
    Update { aggregate_id: A, version: usize },
    Delete { aggregate_id: A },
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
