use crate::map2::{Chunk, ChunkCommand, ChunkFactory, ChunkId, ChunkOperation, TileMapConfig, TileMapError};
use bevy::{platform::sync::RwLock, tasks::BoxedFuture};
use serde::{de::DeserializeOwned, Serialize};
use shine_infra::db::event_source::{
    pg::PgEventDb, Aggregate, AggregateId, Event, EventDb, EventNotification, EventStore, EventStoreError,
    SnapshotStore,
};
use std::{marker::PhantomData, sync::Arc};

impl AggregateId for ChunkId {
    fn from_string(id: String) -> Self {
        let parts: Vec<&str> = id.split('-').collect();
        if parts.len() != 2 {
            panic!("Invalid ChunkId format");
        }
        let x = parts[0]
            .parse::<usize>()
            .expect("Invalid ChunkId format - x coordinate");
        let y = parts[1]
            .parse::<usize>()
            .expect("Invalid ChunkId format - y coordinate");
        ChunkId(x, y)
    }

    fn to_string(&self) -> String {
        format!("{}-{}", self.0, self.1)
    }
}

impl<C> Aggregate for Chunk<C>
where
    C: TileMapConfig,
    C::ChunkOperation: Event,
    Chunk<C>: Default + DeserializeOwned + Serialize,
{
    type Event = C::ChunkOperation;
    type AggregateId = ChunkId;
    const NAME: &'static str = C::NAME;

    fn apply(&mut self, event: &Self::Event) -> Result<(), EventStoreError> {
        event.apply(self);
        Ok(())
    }
}

// command flow:
// 1. Client temporarily applies the command locally
// 2. Client sends command to server using the store_operation
// 3. Poll/Get new commands(events) from server
// 4. Client applies persisted commands locally and finalize the local update
// Note: Commands are persisted independent if it can be applied, server is not checking, clients shpuld apply the same logic and
// skip invalid commands. But server also maintains a replayed state, thus it can
// - detect client drifts
// - send the (new) client the current state
//todo: operation should now when the local operation was persisted and from the server using some unique op id

pub struct PersistedChunkFactory<DB, C>
where
    DB: EventDb<C::ChunkOperation, ChunkId>,
    C: TileMapConfig,
    C::ChunkOperation: Event,
    Chunk<C>: Aggregate<Event = C::ChunkOperation, AggregateId = ChunkId>,
{
    event_db: DB,
    ph: PhantomData<C>,
}

impl<DB, C> PersistedChunkFactory<DB, C>
where
    DB: EventDb<C::ChunkOperation, ChunkId>,
    C: TileMapConfig,
    C::ChunkOperation: Event,
    Chunk<C>: Aggregate<Event = C::ChunkOperation, AggregateId = ChunkId>,
{
    pub fn new(event_db: DB) -> Self {
        Self {
            event_db,
            ph: PhantomData,
        }
    }

    async fn read_chunk(&self, config: &C, chunk_id: ChunkId) -> Result<(Chunk<C>, usize), TileMapError> {
        let mut es = self.event_db.create_context().await?;
        let size = config.chunk_size();
        match es
            .get_aggregate::<Chunk<C>, _>(&chunk_id, move || Chunk::new(size))
            .await
        {
            Ok(aggregate) => {
                let version = aggregate.version();
                let chunk = aggregate.into_aggregate();
                Ok((chunk, version))
            }
            Err(EventStoreError::NotFound) => {
                let chunk = Chunk::new(size);
                Ok((chunk, 0))
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn read_update(&self, chunk_id: ChunkId, version: usize) -> Result<Vec<ChunkCommand<C>>, TileMapError> {
        let mut es = self.event_db.create_context().await?;
        match es.get_events(&chunk_id, Some(version), None).await {
            Ok(events) => Ok(events
                .into_iter()
                .map(|stored_event| ChunkCommand {
                    version: stored_event.version,
                    operation: stored_event.event,
                })
                .collect::<Vec<_>>()),
            Err(EventStoreError::NotFound) => Err(TileMapError::ChunkNotFound),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn store_operation(&self, chunk_id: ChunkId, operation: C::ChunkOperation) -> Result<(), TileMapError> {
        let mut es = self.event_db.create_context().await?;
        es.store_events(&chunk_id, None, &[operation]).await?;
        Ok(())
    }
}

impl<C> PersistedChunkFactory<PgEventDb<C::ChunkOperation, ChunkId>, C>
where
    C: TileMapConfig,
    C::ChunkOperation: Event,
    Chunk<C>: Aggregate<Event = C::ChunkOperation, AggregateId = ChunkId>,
{
    pub async fn start_listen(&self, queue: Arc<RwLock<Vec<ChunkId>>>) -> Result<(), TileMapError> {
        self.event_db
            .listen_to_stream_updates(move |notification| {
                let chunk_id = match notification {
                    EventNotification::Update { aggregate_id, .. } => aggregate_id,
                    EventNotification::Delete { aggregate_id } => aggregate_id,
                    EventNotification::Insert { aggregate_id } => aggregate_id,
                };
                let mut queue = queue.write().unwrap();
                queue.push(chunk_id);
            })
            .await?;
        Ok(())
    }
}

impl<DB, C> ChunkFactory<C> for PersistedChunkFactory<DB, C>
where
    DB: EventDb<C::ChunkOperation, ChunkId>,
    C: TileMapConfig,
    C::ChunkOperation: Event,
    Chunk<C>: Aggregate<Event = C::ChunkOperation, AggregateId = ChunkId>,
{
    fn read<'a>(
        &'a self,
        config: &'a C,
        chunk_id: ChunkId,
    ) -> BoxedFuture<'a, Result<(Chunk<C>, usize), TileMapError>> {
        Box::pin(self.read_chunk(config, chunk_id))
    }

    fn read_updates<'a>(
        &'a self,
        _config: &C,
        chunk_id: ChunkId,
        version: usize,
    ) -> BoxedFuture<'a, Result<Vec<ChunkCommand<C>>, TileMapError>> {
        Box::pin(self.read_update(chunk_id, version))
    }
}
