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
pub struct ChunkId(pub usize, pub usize);

impl ChunkId {
    /// Return the relative axial coordinate of a chunk id.
    /// This function interprets the chunk coordinates as the q,r components of the axial coordinates.
    pub fn relative_axial_coord(&self, id: ChunkId) -> AxialCoord {
        let dx = id.0 as isize - self.0 as isize;
        let dy = id.1 as isize - self.1 as isize;
        AxialCoord::new(dx.try_into().unwrap(), dy.try_into().unwrap())
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
