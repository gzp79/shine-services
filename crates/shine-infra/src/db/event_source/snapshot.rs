use crate::db::event_source::{AggregateId, Event, EventStoreError, StoredEvent};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

pub trait Aggregate: 'static + Serialize + DeserializeOwned + Send + Sync {
    type Event: Event;
    type AggregateId: AggregateId;

    const NAME: &'static str;

    fn apply(&mut self, event: Self::Event) -> Result<(), EventStoreError>;
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
    pub fn new<D>(aggregate_id: A::AggregateId, default: D) -> Self
    where
        D: FnOnce() -> A,
    {
        Self {
            aggregate_id,
            version: 0,
            aggregate: default(),
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

    pub fn apply<I>(&mut self, events: I) -> Result<(), EventStoreError>
    where
        I: IntoIterator<Item = StoredEvent<A::Event>>,
    {
        log::debug!(
            "Applying events to aggregate {:?} at version {}",
            self.aggregate_id,
            self.version
        );

        for event in events {
            if event.version <= self.version {
                continue;
            }
            if event.version > self.version + 1 {
                return Err(EventStoreError::EventOutOfOrder);
            }
            self.aggregate.apply(event.event)?;
            self.version = event.version;
        }

        log::debug!(
            "Applied events to aggregate {:?} up to version {}",
            self.aggregate_id,
            self.version
        );

        Ok(())
    }
}

pub trait SnapshotStore {
    type Event: Event;
    type AggregateId: AggregateId;

    /// Get aggregate up to the latest version using the latest snapshot if present.
    fn get_aggregate<G, D>(
        &mut self,
        aggregate_id: &Self::AggregateId,
        default: D,
    ) -> impl Future<Output = Result<Snapshot<G>, EventStoreError>> + Send
    where
        G: Aggregate<Event = Self::Event, AggregateId = Self::AggregateId>,
        D: FnOnce() -> G + Send + Sync + 'static;

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
