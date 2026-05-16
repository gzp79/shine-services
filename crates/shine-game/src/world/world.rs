use glam::Vec2;

use crate::{
    math::{
        hex::{HexFlatDir, HexPointyDir},
        prng::SplitMix64,
    },
    world::{Chunk, ChunkId},
};
use std::collections::HashMap;

/// The core subdivision depth to align chunks
pub const SUBDIVISION_BASE: u32 = 4;
/// The numbe of cells on the edge of a chunk
pub const SUBDIVISION_COUNT: u32 = 2u32.pow(SUBDIVISION_BASE);

/// The world size (circumcenter) of a chunk (in meter)
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
    pub fn boundary_edge_dual_polygons(&self, _owner_id: ChunkId, _edge_idx: HexFlatDir) -> Option<()> {
        None
    }

    /// Returns dual polygon for boundary vertex cell (single triangular cell).
    /// Format: (vertices, indices, starts) matching Chunk::dual_polygons()
    pub fn boundary_vertex_dual_polygon(&self, owner_id: ChunkId, vertex_idx: HexPointyDir) -> Option<Vec<Vec2>> {
        let v1 = vertex_idx;
        let (n2, v2, n0, v0) = match vertex_idx {
            HexPointyDir::E => (HexFlatDir::NE, HexPointyDir::SW, HexFlatDir::SE, HexPointyDir::NW),
            HexPointyDir::NE => (HexFlatDir::N, HexPointyDir::SE, HexFlatDir::NE, HexPointyDir::W),
            HexPointyDir::NW => (HexFlatDir::NW, HexPointyDir::E, HexFlatDir::N, HexPointyDir::SW),
            HexPointyDir::W => (HexFlatDir::SW, HexPointyDir::NE, HexFlatDir::NW, HexPointyDir::SE),
            HexPointyDir::SW => (HexFlatDir::S, HexPointyDir::NW, HexFlatDir::SW, HexPointyDir::E),
            HexPointyDir::SE => (HexFlatDir::SE, HexPointyDir::W, HexFlatDir::S, HexPointyDir::NE),
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
