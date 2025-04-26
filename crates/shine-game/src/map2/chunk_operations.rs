use crate::map2::{Chunk, TileMapConfig};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;

pub trait ChunkOperation: 'static + Serialize + DeserializeOwned + Send + Sync {
    type TileMapConfig: TileMapConfig;

    fn apply(&self, chunk: &mut Chunk<Self::TileMapConfig>);
    fn apply_local(&self, chunk: &mut Chunk<Self::TileMapConfig>, version: usize);
}

/// Utility struct when no operation is needed on a tile.
#[derive(Serialize, Deserialize)]
pub struct NoOperation<C>
where
    C: TileMapConfig,
{
    ph: PhantomData<C>,
}

impl<C> NoOperation<C>
where
    C: TileMapConfig,
{
    pub fn new() -> Self {
        Self { ph: PhantomData }
    }
}

impl<C> ChunkOperation for NoOperation<C>
where
    C: TileMapConfig,
{
    type TileMapConfig = C;

    fn apply(&self, _chunk: &mut Chunk<Self::TileMapConfig>) {}

    fn apply_local(&self, _chunk: &mut Chunk<Self::TileMapConfig>, _version: usize) {}
}
