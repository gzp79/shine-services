use crate::map2::{Chunk, ChunkId, Tile};

pub trait ChunkOperation: 'static + Send + Sync {
    type Tile: Tile;

    fn apply(&self, chunk: &mut Chunk<Self::Tile>);
}

pub struct ChunkCommand<O>
where
    O: ChunkOperation,
{
    pub chunk_id: ChunkId,
    pub version: usize,
    pub operation: O,
}
