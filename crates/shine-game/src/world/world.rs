use crate::{
    math::{
        hex::{HexNeighbor, HexVertex},
        prng::SplitMix64,
    },
    world::{Chunk, ChunkId},
};
use std::collections::HashMap;

/// The core subdivision depth to align chunks
pub const SUBDIVISION_BASE: u32 = 4;
/// The numbe of cells on the edge of a chunk
pub const SUBDIVISION_COUNT: u32 = 2u32.pow(SUBDIVISION_BASE);

/// The world size of a chunk (in meter)
pub const CHUNK_WORLD_SIZE: f32 = 1000.0;
/// The "ideal" length of the side of a cell (in meter)
pub const CELL_WORLD_SIZE: f32 = CHUNK_WORLD_SIZE / SUBDIVISION_COUNT as f32;

pub struct World {
    rng_seed: SplitMix64,
    chunks: HashMap<ChunkId, Chunk>,
}

impl World {
    pub fn new() -> Self {
        Self {
            rng_seed: SplitMix64::new(),
            chunks: HashMap::new(),
        }
    }

    pub fn init_chunk(&mut self, id: ChunkId) {
        self.chunks.insert(id, Chunk::new(&self.rng_seed, id));
    }

    pub fn chunk(&self, id: ChunkId) -> Option<&Chunk> {
        self.chunks.get(&id)
    }

    pub fn remove_chunk(&mut self, id: ChunkId) {
        self.chunks.remove(&id);
    }

    pub fn chunk_vertices(&self, id: ChunkId) -> Vec<f32> {
        self.chunk(id).map(|c| c.quad_vertices()).unwrap_or_default()
    }

    pub fn chunk_quad_indices(&self, id: ChunkId) -> Vec<u32> {
        self.chunk(id).map(|c| c.quad_indices()).unwrap_or_default()
    }

    pub fn chunk_boundary_indices(&self, id: ChunkId) -> Vec<u32> {
        self.chunk(id).map(|c| c.boundary_indices()).unwrap_or_default()
    }

    pub fn chunk_world_offset(&self, reference: ChunkId, target: ChunkId) -> Vec<f32> {
        if self.chunk(reference).is_none() {
            return vec![];
        }
        let offset = reference.relative_world_position(target);
        vec![offset.x, offset.y]
    }

    /// Returns dual polygons for boundary edge cells owned by the given chunk.
    /// Format: (vertices, indices, starts) matching Chunk::dual_polygons()
    pub fn boundary_edge_dual_polygons(&self, _owner_id: ChunkId, _edge_idx: HexNeighbor) -> Option<()> {
        None
    }

    /// Returns dual polygon for boundary vertex cell (single triangular cell).
    /// Format: (vertices, indices, starts) matching Chunk::dual_polygons()
    pub fn boundary_vertex_dual_polygon(&self, owner_id: ChunkId, vertex_idx: HexVertex) -> Option<()> {
        let (n1, n2) = match vertex_idx {
            HexVertex::NNW => (HexNeighbor::NW, HexNeighbor::N),
            HexVertex::NNE => (HexNeighbor::N, HexNeighbor::NE),
            HexVertex::E => (HexNeighbor::NE, HexNeighbor::SE),
            HexVertex::SSE => (HexNeighbor::SE, HexNeighbor::S),
            HexVertex::SSW => (HexNeighbor::S, HexNeighbor::SW),
            HexVertex::W => (HexNeighbor::SW, HexNeighbor::NW),
        };

        let _n1 = self.chunk(owner_id.neighbor(n1))?;
        let _n2 = self.chunk(owner_id.neighbor(n2))?;

        None
    }
}
