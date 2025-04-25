use crate::db::event_source::{AggregateId, Event, EventStoreError, StoredEvent};
use serde::{Deserialize, Serialize};
use std::future::Future;

pub trait Aggregate: 'static + Default + Serialize + for<'de> Deserialize<'de> + Send + Sync {
    type Event: Event;
    type AggregateId: ToString;

    const NAME: &'static str;

    fn apply(&mut self, event: &Self::Event) -> Result<(), EventStoreError>;
}

#[derive(Debug, Clone)]
pub struct Snapshot<A>
where
    A: Aggregate,
{
    aggregate_id: A::AggregateId,
    version: usize,
    aggregate: A,
}

impl<A> Snapshot<A>
where
    A: Aggregate,
{
    pub fn new(aggregate_id: A::AggregateId) -> Self {
        Self {
            aggregate_id,
            version: 0,
            aggregate: Default::default(),
        }
    }

    pub fn new_from_data(aggregate_id: A::AggregateId, version: usize, data: &str) -> Result<Self, EventStoreError> {
        let aggregate = serde_json::from_str(data).map_err(EventStoreError::EventSerialization)?;

        Ok(Self {
            aggregate_id,
            version,
            aggregate,
        })
    }

    pub fn version(&self) -> usize {
        self.version
    }

    pub fn id(&self) -> &A::AggregateId {
        &self.aggregate_id
    }

    pub fn aggregate(&self) -> &A {
        &self.aggregate
    }

    pub fn into_aggregate(self) -> A {
        self.aggregate
    }

    pub fn apply(&mut self, events: &[StoredEvent<A::Event>]) -> Result<(), EventStoreError> {
        for event in events {
            if event.version <= self.version {
                continue;
            }
            if event.version > self.version + 1 {
                return Err(EventStoreError::EventOutOfOrder);
            }
            self.aggregate.apply(&event.event)?;
            self.version = event.version;
        }

        Ok(())
    }
}

pub trait SnapshotStore {
    type Event: Event;
    type AggregateId: AggregateId;

    /// Get aggregate up to the latest version using the latest snapshot if present.
    fn get_aggregate<G>(
        &mut self,
        aggregate_id: &Self::AggregateId,
    ) -> impl Future<Output = Result<Option<Snapshot<G>>, EventStoreError>> + Send
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>;

    /// Get the last stored aggregate
    fn get_snapshot<G>(
        &mut self,
        aggregate_id: &Self::AggregateId,
    ) -> impl Future<Output = Result<Option<Snapshot<G>>, EventStoreError>> + Send
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>;

    /// Store a new snapshot for an aggregate
    fn store_snapshot<G>(&mut self, snapshot: &Snapshot<G>) -> impl Future<Output = Result<(), EventStoreError>> + Send
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>;
}
