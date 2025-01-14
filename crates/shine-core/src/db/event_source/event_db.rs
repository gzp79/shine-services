use crate::db::event_source::{Event, EventStore, EventStoreError, SnapshotStore};
use std::future::Future;
use uuid::Uuid;

pub trait EventDbContext<'c, E>: EventStore<Event = E> + SnapshotStore<Event = E> + Send
where
    E: Event,
{
}

#[derive(Debug, Clone)]
pub struct EventNotification {
    pub aggregate_id: Uuid,
    pub version: usize,
}

pub trait EventDb<E>: Send + Sync
where
    E: Event,
{
    fn create_context(&self) -> impl Future<Output = Result<impl EventDbContext<'_, E>, EventStoreError>> + Send;

    fn listen_to_events<F>(&self, handler: F) -> impl Future<Output = Result<(), EventStoreError>> + Send
    where
        F: Fn(EventNotification) -> () + Send + Sync + 'static;

    fn unlisten_to_events(&self) -> impl Future<Output = Result<(), EventStoreError>> + Send;
}
