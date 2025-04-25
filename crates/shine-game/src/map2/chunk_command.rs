use crate::map2::{Chunk, ChunkId, TileMapConfig};
use serde::{de::DeserializeOwned, Serialize};

pub trait ChunkOperation: 'static + Serialize + DeserializeOwned + Send + Sync {
    type TileMapConfig: TileMapConfig;

    fn apply(&self, chunk: &mut Chunk<Self::TileMapConfig>);
    fn apply_local(&self, chunk: &mut Chunk<Self::TileMapConfig>);
}

pub struct ChunkCommand<O>
where
    O: ChunkOperation,
{
    pub chunk_id: ChunkId,
    pub version: Option<usize>,
    pub operation: O,
}

impl<O> ChunkCommand<O>
where
    O: ChunkOperation,
{
    pub fn new(chunk_id: ChunkId, version: Option<usize>, operation: O) -> Self {
        Self {
            chunk_id,
            version,
            operation,
        }
    }

    pub fn new_local(chunk_id: ChunkId, operation: O) -> Self {
        Self {
            chunk_id,
            version: None,
            operation,
        }
    }
}
