use crate::map2::{Chunk, ChunkId, TileMapConfig};
use bevy::tasks::BoxedFuture;

pub trait ChunkFactory<C>: 'static + Send + Sync
where
    C: TileMapConfig,
{
    fn read<'a>(&'a self, config: &C, chunk_id: ChunkId) -> BoxedFuture<'a, Result<Chunk<C>, ()>>;
}
