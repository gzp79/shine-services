use serde::{Deserialize, Serialize};
use std::future::Future;
use uuid::Uuid;

use super::{Event, EventStoreError, StoredEvent};

pub trait Aggregate: 'static + Default + Serialize + for<'de> Deserialize<'de> + Send + Sync {
    type Event: Event;

    const NAME: &'static str;

    fn apply(&mut self, event: &Self::Event) -> Result<(), EventStoreError>;
}

#[derive(Debug, Clone)]
pub struct Snapshot<A>
where
    A: Aggregate,
{
    aggregate_id: Uuid,
    version: usize,
    aggregate: A,
}

impl<A> Snapshot<A>
where
    A: Aggregate,
{
    pub fn new(aggregate_id: Uuid) -> Self {
        Self {
            aggregate_id,
            version: 0,
            aggregate: Default::default(),
        }
    }

    pub fn new_from_data(aggregate_id: Uuid, version: usize, data: &str) -> Result<Self, EventStoreError> {
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

    pub fn id(&self) -> &Uuid {
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

    /// Get aggregate up to the latest version using the latest snapshot if present.
    fn get_aggregate<A>(
        &mut self,
        aggregate_id: &Uuid,
    ) -> impl Future<Output = Result<Option<Snapshot<A>>, EventStoreError>> + Send
    where
        A: Aggregate<Event = Self::Event>;

    /// Get the last stored aggregate
    fn get_snapshot<A>(
        &mut self,
        aggregate_id: &Uuid,
    ) -> impl Future<Output = Result<Option<Snapshot<A>>, EventStoreError>> + Send
    where
        A: Aggregate<Event = Self::Event>;

    /// Store a new snapshot for an aggregate
    fn store_snapshot<A>(&mut self, snapshot: &Snapshot<A>) -> impl Future<Output = Result<(), EventStoreError>> + Send
    where
        A: Aggregate<Event = Self::Event>;
}
