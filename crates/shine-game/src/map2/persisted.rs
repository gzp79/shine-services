use crate::map2::{Chunk, ChunkFactory, ChunkId, ChunkOperation, TileMapConfig};
use bevy::tasks::BoxedFuture;
use serde::{de::DeserializeOwned, Serialize};
use shine_infra::db::event_source::{
    Aggregate, AggregateId, Event, EventDb, EventStore, EventStoreError, SnapshotStore,
};
use std::marker::PhantomData;

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

    async fn read_chunk(&self, chunk_id: ChunkId) -> Result<Chunk<C>, ()> {
        let mut es = self.event_db.create_context().await.map_err(|_| ())?;
        let chunk = es
            .get_aggregate::<Chunk<C>>(&chunk_id)
            .await
            .map_err(|_| ())?
            .map(|aggregate| {
                let version = aggregate.version();
                let mut chunk = aggregate.into_aggregate();
                chunk.set_version(version);
                chunk
            })
            .unwrap_or_default();
        Ok(chunk)
    }

    pub async fn store_operation(&self, chunk_id: ChunkId, operation: C::ChunkOperation) -> Result<(), ()> {
        let mut es = self.event_db.create_context().await.map_err(|_| ())?;
        es.store_events(&chunk_id, None, &[operation]).await.map_err(|_| ())?;
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
    fn read<'a>(&'a self, _config: &C, chunk_id: ChunkId) -> BoxedFuture<'a, Result<Chunk<C>, ()>> {
        Box::pin(self.read_chunk(chunk_id))
    }
}
