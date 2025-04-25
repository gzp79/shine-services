use crate::map2::{ChunkOperation, Tile};

#[derive(Clone, Debug)]
pub struct ChunkSizes {
    pub inner_width: usize,
    pub inner_height: usize,
    pub side_width: usize,
    pub side_height: usize,
}

pub trait TileMapConfig: 'static + Clone + Send + Sync {
    type Tile: Tile;
    type ChunkOperation: ChunkOperation<Tile = Self::Tile>;

    fn chunk_size(&self) -> ChunkSizes;
}
