use crate::map2::{
    ChunkCommand, ChunkFactory, ChunkId, ChunkOperation, ChunkStore, PersistedChunkCommand, TileMapConfig,
    TileMapError, UpdatedChunks,
};
use bevy::{
    platform::sync::{Arc, Mutex},
    tasks::BoxedFuture,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shine_infra::db::{
    event_source::{pg::PgEventDb, Aggregate, AggregateStore, Event, EventDb, EventSourceError, EventStore},
    PGConnectionPool,
};

#[allow(type_alias_bounds)]
pub type ESChunkDB<C: TileMapConfig> = PgEventDb<C::PersistedChunkOperation, ChunkId>;

/// Wrapper for the a ChunkStore to be used as an aggregate
#[derive(Serialize, Deserialize)]
#[serde(bound(deserialize = "C::PersistedChunkStore: Serialize + DeserializeOwned"))]
#[serde(rename_all = "camelCase")]
struct ESChunkAggregate<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
{
    data: C::PersistedChunkStore,
}

impl<C> ESChunkAggregate<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Serialize + DeserializeOwned,
{
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            data: C::PersistedChunkStore::new(width, height),
        }
    }

    pub fn into_chunk(self) -> C::PersistedChunkStore {
        self.data
    }
}

impl<C> Aggregate for ESChunkAggregate<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Serialize + DeserializeOwned,
{
    const NAME: &'static str = C::NAME;
    type Event = C::PersistedChunkOperation;
    type AggregateId = ChunkId;

    fn apply(&mut self, event: Self::Event) -> Result<(), EventSourceError> {
        event.apply(&mut self.data);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ESChunkFactory<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Serialize + DeserializeOwned,
{
    event_db: Arc<ESChunkDB<C>>,
}

impl<C> ESChunkFactory<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Serialize + DeserializeOwned,
{
    pub async fn new(pg_pool: &PGConnectionPool) -> Result<Self, EventSourceError> {
        let event_db = ESChunkDB::<C>::new(pg_pool).await?;
        Ok(Self { event_db: Arc::new(event_db) })
    }

    fn create_chunk(size: (usize, usize)) -> ESChunkAggregate<C> {
        let (w, h) = size;
        ESChunkAggregate::new(w, h)
    }

    pub async fn read_chunk(
        &self,
        config: &C,
        chunk_id: ChunkId,
    ) -> Result<(C::PersistedChunkStore, usize), TileMapError> {
        let mut es = self.event_db.create_context().await?;
        let size = config.chunk_size();
        match es.get_aggregate_with(&chunk_id, move || Self::create_chunk(size)).await {
            Ok(aggregate) => {
                let version = aggregate.version;
                let chunk = aggregate.aggregate.into_chunk();
                Ok((chunk, version))
            }
            Err(EventSourceError::StreamNotFound) => Ok((Self::create_chunk(size).into_chunk(), 0)),
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
            Err(EventSourceError::StreamNotFound) => Err(TileMapError::ChunkNotFound),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn listen_changes(&self, _config: &C, queue: Arc<Mutex<UpdatedChunks>>) -> Result<(), TileMapError> {
        self.event_db
            .listen_to_stream_updates(move |notification| {
                let chunk_id = *notification.aggregate_id();
                {
                    let mut queue = queue.lock().unwrap();
                    if queue.insert(chunk_id) {
                        log::trace!("Chunk {:?} was updated", chunk_id);
                    }
                }
            })
            .await?;
        Ok(())
    }

    pub async fn store_operation<O>(&self, chunk_id: ChunkId, operation: O) -> Result<(), TileMapError>
    where
        O: Into<C::PersistedChunkOperation>,
    {
        let mut es = self.event_db.create_context().await?;
        es.unchecked_store_events(&chunk_id, &[operation.into()]).await?;
        Ok(())
    }
}

impl<C> ChunkFactory<C> for ESChunkFactory<C>
where
    C: TileMapConfig,
    C::PersistedChunkOperation: Event,
    C::PersistedChunkStore: Serialize + DeserializeOwned,
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
