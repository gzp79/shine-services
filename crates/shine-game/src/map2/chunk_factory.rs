use crate::map2::{Chunk, Tile};
use bevy::tasks::BoxedFuture;

pub trait ChunkFactory<T>: 'static + Send + Sync
where
    T: Tile,
{
    fn read(&self, width: usize, height: usize) -> BoxedFuture<'_, Result<Chunk<T>, ()>>;
}
