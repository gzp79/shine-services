use crate::map2::{ChunkOperation, Tile};

#[derive(Clone, Copy, Debug)]
pub struct ChunkSize {
    pub width: usize,
    pub height: usize,
}

impl ChunkSize {
    #[inline]
    pub fn area(&self) -> usize {
        self.width * self.height
    }
}

pub trait TileMapConfig: 'static + Clone + Send + Sync {
    const NAME: &'static str;
    type Tile: Tile;
    type ChunkOperation: ChunkOperation<TileMapConfig = Self>;

    fn chunk_size(&self) -> ChunkSize;
    fn max_retry_count(&self) -> usize;
}
