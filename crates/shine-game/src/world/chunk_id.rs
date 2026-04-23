use crate::{
    math::{
        hex::{AxialCoord, HexNeighbor},
        prng::hash_u32_2,
    },
    world::CHUNK_WORLD_SIZE,
};
use glam::Vec2;

/// Unique identifier of a chunk of the map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub i32, pub i32);

impl ChunkId {
    pub const ORIGIN: ChunkId = ChunkId(0, 0);

    /// World-space offset from `self` to `other`.
    pub fn relative_world_position(&self, other: ChunkId) -> Vec2 {
        let rel = AxialCoord::new(other.0 - self.0, other.1 - self.1);
        rel.center_position(CHUNK_WORLD_SIZE)
    }

    /// Deterministic 32-bit hash from chunk coordinates.
    /// Uses golden-ratio mixing + murmur3 finalizer for good avalanche.
    pub fn hash32(&self) -> u32 {
        hash_u32_2(self.0 as u32, self.1 as u32)
    }

    pub fn id_64(&self) -> u64 {
        let high = self.0 as u64;
        let low = self.1 as u64;
        (high << 32) | low
    }

    pub fn neighbor(&self, direction: HexNeighbor) -> ChunkId {
        AxialCoord::from(*self).neighbor(direction).into()
    }
}

impl From<ChunkId> for AxialCoord {
    fn from(id: ChunkId) -> Self {
        AxialCoord::new(id.0, id.1)
    }
}

impl From<AxialCoord> for ChunkId {
    fn from(c: AxialCoord) -> Self {
        ChunkId(c.q, c.r)
    }
}
