use bevy::ecs::resource::Resource;
use serde::{Deserialize, Serialize};
use shine_forge::map::{
    ChunkCommandQueue, ChunkHashTrack, ChunkHasher, ChunkLayer, ChunkLayerSetup, ChunkOperation, DenseGrid,
    DenseGridChunk, GridChunk, GridChunkTypes, SparseGrid, SparseGridChunk,
};

pub struct GridU8Types;
impl GridChunkTypes for GridU8Types {
    type Tile = u8;

    fn name() -> &'static str {
        "GridU8"
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GridU8Operation {
    SetTile(usize, usize, u8),
}

impl<T> ChunkOperation<T> for GridU8Operation
where
    T: GridChunk<Tile = u8>,
{
    fn check_precondition(&self, _chunk: &T) -> bool {
        true
    }

    fn apply(self, chunk: &mut T) {
        match self {
            GridU8Operation::SetTile(x, y, tile) => {
                *chunk.get_mut(x, y) = tile;
            }
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct DenseGridU8Hasher;

impl<T> ChunkHasher<T> for DenseGridU8Hasher
where
    T: DenseGridChunk<Tile = u8>,
{
    type Hash = u64;

    fn hash(&self, chunk: &T) -> Self::Hash {
        chunk
            .data()
            .iter()
            .fold(0, |acc, &tile| acc.wrapping_mul(31).wrapping_add(tile as u64))
    }
}

#[derive(Resource, Clone, Default)]
pub struct SparseGridU8Hasher;

impl<T> ChunkHasher<T> for SparseGridU8Hasher
where
    T: SparseGridChunk<Tile = u8>,
{
    type Hash = u64;

    fn hash(&self, chunk: &T) -> Self::Hash {
        chunk.occupied().fold(0, |acc, (x, y, tile)| {
            acc.wrapping_mul(31).wrapping_add((x + y + *tile as usize) as u64)
        })
    }
}

pub type DenseGridU8 = DenseGrid<GridU8Types>;
pub type DenseGridU8Layer = ChunkLayer<DenseGridU8>;
pub type DenseGridU8HashTracker = ChunkHashTrack<DenseGridU8, DenseGridU8Hasher>;
pub type DenseGridU8CommandQueue = ChunkCommandQueue<DenseGridU8, GridU8Operation, DenseGridU8Hasher>;
pub type DenseGridU8LayerSetup = ChunkLayerSetup<DenseGridU8, GridU8Operation, DenseGridU8Hasher>;

pub type SparseGridU8 = SparseGrid<GridU8Types>;
pub type SparseGridU8Layer = ChunkLayer<SparseGridU8>;
pub type SparseGridU8HashTracker = ChunkHashTrack<SparseGridU8, DenseGridU8Hasher>;
pub type SparseGridU8CommandQueue = ChunkCommandQueue<SparseGridU8, GridU8Operation, SparseGridU8Hasher>;
pub type SparseGridU8LayerSetup = ChunkLayerSetup<SparseGridU8, GridU8Operation, SparseGridU8Hasher>;
