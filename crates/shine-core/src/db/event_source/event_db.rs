use crate::db::event_source::{Event, EventStore, SnapshotStore, EventStoreError};
use std::future::Future;

pub trait EventDbContext<'c, E>: EventStore<Event = E> + SnapshotStore<Event = E> + Send
where
    E: Event,
{
}

pub trait EventDb<E>: Send + Sync
where
    E: Event,
{
    fn create_context(&self) -> impl Future<Output = Result<impl EventDbContext<'_, E>, EventStoreError>> + Send;
}
