use crate::{
    indexed::IdxVec,
    math::{
        hex::{AxialCoord, LatticeMesher},
        mesh::{QuadTopology, VertIdx},
        rand::Xorshift32,
    },
    world::{CHUNK_WORLD_SIZE, SUBDIVISION_BASE},
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
        let a = self.0 as u32;
        let b = self.1 as u32;
        let mut h = a.wrapping_mul(0x9e3779b9).wrapping_add(b);
        h ^= h >> 16;
        h = h.wrapping_mul(0x45d9f3b);
        h ^= h >> 16;
        h
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

pub struct Chunk {
    pub topology: QuadTopology,
    pub vertices: IdxVec<VertIdx, Vec2>,
}

impl Chunk {
    pub fn new(id: ChunkId) -> Self {
        let rng = Xorshift32::new(id.hash32());
        let mesh = LatticeMesher::new(SUBDIVISION_BASE, rng)
            .with_world_size(CHUNK_WORLD_SIZE)
            .generate();
        let (topology, vertices) = mesh.into_parts();
        Self { topology, vertices }
    }
}
