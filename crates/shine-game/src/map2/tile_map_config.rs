use crate::map2::{scopes, Chunk, ChunkCommand, ChunkOperation, ChunkStore, ChunkUpdates, ChunkVersion, Tile};

pub trait TileMapConfig: 'static + Clone + Send + Sync {
    const NAME: &'static str;
    type Tile: Tile;

    type PersistedChunkStore: ChunkStore<Tile = Self::Tile>;
    type PersistedChunkOperation: ChunkOperation<Tile = Self::Tile>;

    fn chunk_size(&self) -> (usize, usize);
    fn max_retry_count(&self) -> usize;
}

pub type PersistedVersion = ChunkVersion<scopes::Persisted>;
#[allow(type_alias_bounds)]
pub type PersistedChunk<C: TileMapConfig> = Chunk<scopes::Persisted, C::PersistedChunkStore>;
#[allow(type_alias_bounds)]
pub type PersistedChunkCommand<C: TileMapConfig> = ChunkCommand<C::PersistedChunkOperation>;
#[allow(type_alias_bounds)]
pub type PersistedChunkUpdate<C: TileMapConfig> = ChunkUpdates<scopes::Persisted, C::PersistedChunkOperation>;
