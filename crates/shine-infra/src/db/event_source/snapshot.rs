use crate::db::event_source::{
    Aggregate, AggregateStore, EventSourceError, EventStore, StoredAggregate, StoredEvent,
};

/// Helper to replay events from the event store and apply them to an aggregate.
#[derive(Debug, Clone)]
pub struct Snapshot<A>
where
    A: Aggregate,
{
    pub stream_id: A::StreamId,
    pub start_version: usize,
    pub version: usize,
    pub aggregate: A,
}

impl<A> From<StoredAggregate<A>> for Snapshot<A>
where
    A: Aggregate,
{
    fn from(stored_aggregate: StoredAggregate<A>) -> Self {
        Self {
            stream_id: stored_aggregate.stream_id,
            start_version: stored_aggregate.start_version,
            version: stored_aggregate.version,
            aggregate: stored_aggregate.aggregate,
        }
    }
}

impl<A> Snapshot<A>
where
    A: Aggregate,
{
    pub fn new(stream_id: A::StreamId, aggregate: A) -> Self {
        Self {
            stream_id,
            start_version: 0,
            version: 0,
            aggregate,
        }
    }

    pub async fn load_from<DB>(
        db: &mut DB,
        stream_id: &A::StreamId,
        version: Option<usize>,
        init: A,
    ) -> Result<Self, EventSourceError>
    where
        DB: EventStore<Event = A::Event, StreamId = A::StreamId>
            + AggregateStore<Event = A::Event, StreamId = A::StreamId>,
    {
        let snapshot = db
            .get_aggregate::<A>(stream_id, version)
            .await?
            .map(Snapshot::from);
        let mut snapshot = snapshot.unwrap_or_else(|| Snapshot::new(stream_id.clone(), init));
        snapshot.start_version = snapshot.version;

        snapshot.update_from(db, version).await?;
        Ok(snapshot)
    }

    pub fn aggregate_id(&self) -> &str {
        A::NAME
    }

    pub async fn update_from<DB>(
        &mut self,
        db: &mut DB,
        version: Option<usize>,
    ) -> Result<(), EventSourceError>
    where
        DB: EventStore<Event = A::Event, StreamId = A::StreamId>
            + AggregateStore<Event = A::Event, StreamId = A::StreamId>,
    {
        if version.unwrap_or(usize::MAX) > self.version {
            let events = db
                .get_events(&self.stream_id, Some(self.version), version)
                .await?;
            self.apply(events)?;
        }
        Ok(())
    }

    pub fn apply<I>(&mut self, events: I) -> Result<(), EventSourceError>
    where
        I: IntoIterator<Item = StoredEvent<A::Event>>,
    {
        log::debug!(
            "Applying events to aggregate {:?} at version {}",
            self.stream_id,
            self.version
        );

        for event in events {
            if event.version <= self.version {
                continue;
            }
            if event.version > self.version + 1 {
                return Err(EventSourceError::EventOutOfOrder);
            }
            self.aggregate.apply(event.event)?;
            self.version = event.version;
        }

        log::debug!(
            "Applied events to aggregate {:?} up to version {}",
            self.stream_id,
            self.version
        );

        Ok(())
    }
}
