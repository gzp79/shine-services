use bevy::ecs::resource::Resource;
use serde::{Deserialize, Serialize};
use shine_game::{
    hex::AxialCoord,
    map::{
        hex::{DenseHex, DenseHexChunk, HexChunk, HexChunkLayerSetup, HexChunkTypes, HexConfig},
        ChunkCommandQueue, ChunkHashTrack, ChunkHasher, ChunkLayer, ChunkOperation, MapConfig,
    },
};

#[derive(Resource, Clone)]
pub struct TestHexConfig {
    pub radius: u32,
}

impl MapConfig for TestHexConfig {}

impl HexConfig for TestHexConfig {
    fn radius(&self) -> u32 {
        self.radius
    }
}

pub struct HexU8Types;
impl HexChunkTypes for HexU8Types {
    type Tile = u8;

    fn name() -> &'static str {
        "HexU8"
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HexU8Operation {
    SetTile(AxialCoord, u8),
}

impl<T> ChunkOperation<T> for HexU8Operation
where
    T: HexChunk<Tile = u8>,
{
    fn check_precondition(&self, _chunk: &T) -> bool {
        true
    }

    fn apply(self, chunk: &mut T) {
        match self {
            HexU8Operation::SetTile(coord, tile) => {
                *chunk.get_mut(&coord) = tile;
            }
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct DenseHexU8Hasher;

impl<T> ChunkHasher<T> for DenseHexU8Hasher
where
    T: DenseHexChunk<Tile = u8>,
{
    type Hash = u64;

    fn hash(&self, chunk: &T) -> Self::Hash {
        chunk
            .data()
            .iter()
            .fold(0, |acc, &tile| acc.wrapping_mul(31).wrapping_add(tile as u64))
    }
}

pub type DenseHexU8 = DenseHex<HexU8Types>;
pub type DenseHexU8Layer = ChunkLayer<DenseHexU8>;
pub type DenseHexU8HashTracker = ChunkHashTrack<DenseHexU8, DenseHexU8Hasher>;
pub type DenseHexU8CommandQueue = ChunkCommandQueue<DenseHexU8, HexU8Operation, DenseHexU8Hasher>;
pub type DenseHexU8LayerSetup = HexChunkLayerSetup<DenseHexU8, HexU8Operation, DenseHexU8Hasher>;
