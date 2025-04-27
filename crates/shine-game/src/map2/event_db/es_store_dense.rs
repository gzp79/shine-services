use crate::map2::{
    ChunkCommand, ChunkFactory, ChunkId, ChunkStore, PersistedChunkCommand, TileMapConfig, TileMapError, UpdatedChunks,
};
use bevy::{
    platform::sync::{Arc, Mutex},
    tasks::BoxedFuture,
};
use shine_infra::db::event_source::{
    pg::PgEventDb, Aggregate, Event, EventDb, EventNotification, EventStore, EventStoreError, SnapshotStore,
};
use std::marker::PhantomData;

#[allow(type_alias_bounds)]
pub type ESChunkDB<C: TileMapConfig> = PgEventDb<C::PersistedChunkOperation, ChunkId>;

pub struct ESChunkFactory<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Aggregate<Event = C::PersistedChunkOperation, AggregateId = ChunkId>,
{
    event_db: ESChunkDB<C>,
    ph: PhantomData<C>,
}

impl<C> ESChunkFactory<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Aggregate<Event = C::PersistedChunkOperation, AggregateId = ChunkId>,
{
    pub fn new(event_db: ESChunkDB<C>) -> Self {
        Self {
            event_db,
            ph: PhantomData,
        }
    }

    fn create_chunk(size: (usize, usize)) -> C::PersistedChunkStore {
        let (w, h) = size;
        <C::PersistedChunkStore as ChunkStore>::new(w, h)
    }

    pub async fn read_chunk(
        &self,
        config: &C,
        chunk_id: ChunkId,
    ) -> Result<(C::PersistedChunkStore, usize), TileMapError> {
        let mut es = self.event_db.create_context().await?;
        let size = config.chunk_size();
        match es.get_aggregate(&chunk_id, move || Self::create_chunk(size)).await {
            Ok(aggregate) => {
                let version = aggregate.version();
                let chunk = aggregate.into_aggregate();
                Ok((chunk, version))
            }
            Err(EventStoreError::NotFound) => Ok((Self::create_chunk(size), 0)),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn read_updates(
        &self,
        _config: &C,
        chunk_id: ChunkId,
        version: usize,
    ) -> Result<Vec<PersistedChunkCommand<C>>, TileMapError> {
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

    pub async fn listen_changes(&self, _config: &C, queue: Arc<Mutex<UpdatedChunks>>) -> Result<(), TileMapError> {
        self.event_db
            .listen_to_stream_updates(move |notification| {
                let chunk_id = match notification {
                    EventNotification::Update { aggregate_id, .. } => aggregate_id,
                    EventNotification::Delete { aggregate_id } => aggregate_id,
                    EventNotification::Insert { aggregate_id } => aggregate_id,
                };
                {
                    let mut queue = queue.lock().unwrap();
                    queue.push(chunk_id);
                }
            })
            .await?;
        Ok(())
    }

    pub async fn store_operation(
        &self,
        chunk_id: ChunkId,
        operation: C::PersistedChunkOperation,
    ) -> Result<(), TileMapError> {
        let mut es = self.event_db.create_context().await?;
        es.store_events(&chunk_id, None, &[operation]).await?;
        Ok(())
    }
}

impl<C> ChunkFactory<C> for ESChunkFactory<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Aggregate<Event = C::PersistedChunkOperation, AggregateId = ChunkId>,
{
    fn read<'a>(
        &'a self,
        config: &'a C,
        chunk_id: ChunkId,
    ) -> BoxedFuture<'a, Result<(C::PersistedChunkStore, usize), TileMapError>> {
        Box::pin(self.read_chunk(config, chunk_id))
    }

    fn read_updates<'a>(
        &'a self,
        config: &'a C,
        chunk_id: ChunkId,
        version: usize,
    ) -> BoxedFuture<'a, Result<Vec<PersistedChunkCommand<C>>, TileMapError>> {
        Box::pin(self.read_updates(config, chunk_id, version))
    }

    fn listen_updates<'a>(
        &'a self,
        config: &'a C,
        channel: Arc<Mutex<UpdatedChunks>>,
    ) -> BoxedFuture<'a, Result<(), TileMapError>> {
        Box::pin(self.listen_changes(config, channel))
    }
}
