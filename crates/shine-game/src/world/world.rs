use glam::Vec2;

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
    pub fn boundary_vertex_dual_polygon(&self, owner_id: ChunkId, vertex_idx: HexVertex) -> Option<Vec<Vec2>> {
        let v1 = vertex_idx;
        let (n2, v2, n0, v0) = match vertex_idx {
            HexVertex::E => (HexNeighbor::NE, HexVertex::SSW, HexNeighbor::SE, HexVertex::NNW),
            HexVertex::NNE => (HexNeighbor::N, HexVertex::SSE, HexNeighbor::NE, HexVertex::W),
            HexVertex::NNW => (HexNeighbor::NW, HexVertex::E, HexNeighbor::N, HexVertex::SSW),
            HexVertex::W => (HexNeighbor::SW, HexVertex::NNE, HexNeighbor::NW, HexVertex::SSE),
            HexVertex::SSW => (HexNeighbor::S, HexVertex::NNW, HexNeighbor::SW, HexVertex::E),
            HexVertex::SSE => (HexNeighbor::SE, HexVertex::W, HexNeighbor::S, HexVertex::NNE),
        };

        let id0 = owner_id.neighbor(n0);
        let id1 = owner_id;
        let id2 = owner_id.neighbor(n2);

        let chunk0 = self.chunk(id0)?;
        let chunk1 = self.chunk(id1)?;
        let chunk2 = self.chunk(id2)?;

        let mut vertices = Vec::new();

        for (id, chunk, corner) in [(id0, chunk0, v0), (id1, chunk1, v1), (id2, chunk2, v2)] {
            let offset = owner_id.relative_world_position(id);
            let vi = chunk.boundary_corner_vertex(corner);
            for q in chunk.mesh.boundary_dual_vertices(vi) {
                let pos = chunk.mesh.dual_p(q).unwrap();
                vertices.push(pos + offset);
            }
        }

        Some(vertices)
    }
}
