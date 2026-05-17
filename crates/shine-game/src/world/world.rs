use glam::Vec2;

use crate::{
    math::{
        hex::{HexFlatDir, HexPointyDir},
        prng::SplitMix64,
        quadrangulation::VertexIndex,
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

    /// Returns dual polygons for the non-corner vertices along a chunk edge.
    /// Each such vertex lies on the shared boundary between two chunks, so its dual polygon
    /// is built from finite quads in both the owner and the neighbor chunk.
    /// Corner vertices are excluded — they are covered by `boundary_vertex_dual_polygon`.
    pub fn boundary_edge_dual_polygons(&self, owner_id: ChunkId, edge_idx: HexFlatDir) -> Option<Vec<Vec<Vec2>>> {
        let (neighbor_dir, neighbor_edge) = match edge_idx {
            HexFlatDir::NE => (HexFlatDir::NE, HexFlatDir::SW),
            HexFlatDir::N => (HexFlatDir::N, HexFlatDir::S),
            HexFlatDir::NW => (HexFlatDir::NW, HexFlatDir::SE),
            HexFlatDir::SW => (HexFlatDir::SW, HexFlatDir::NE),
            HexFlatDir::S => (HexFlatDir::S, HexFlatDir::N),
            HexFlatDir::SE => (HexFlatDir::SE, HexFlatDir::NW),
        };

        let neighbor_id = owner_id.neighbor(neighbor_dir);
        let owner = self.chunk(owner_id)?;
        let neighbor = self.chunk(neighbor_id)?;
        let neighbor_offset = owner_id.relative_world_position(neighbor_id);

        // Collect neighbor inner vertices reversed: pop both corners, reverse in-place.
        // The neighbor edge runs in the opposite direction to the owner edge, so reversing
        // aligns vertex positions pairwise.
        let inner_neighbor: Vec<VertexIndex> = {
            let mut v: Vec<_> = neighbor.boundary_edge_vertices(neighbor_edge).collect();
            v.pop();
            v.reverse();
            v.pop();
            v
        };
        let inner_owner = owner
            .boundary_edge_vertices(edge_idx)
            .skip(1)
            .take(inner_neighbor.len());

        let mut polygons = Vec::with_capacity(inner_neighbor.len());
        for (vi_owner, vi_neighbor) in inner_owner.zip(inner_neighbor) {
            let mut vertices = Vec::new();
            for q in owner.mesh.boundary_dual_vertices(vi_owner) {
                vertices.push(owner.mesh.dual_p(q).unwrap());
            }
            for q in neighbor.mesh.boundary_dual_vertices(vi_neighbor) {
                vertices.push(neighbor.mesh.dual_p(q).unwrap() + neighbor_offset);
            }
            polygons.push(vertices);
        }

        Some(polygons)
    }

    /// Returns dual polygon for boundary vertex cell (single triangular cell).
    /// Format: (vertices, indices, starts) matching Chunk::dual_polygons()
    pub fn boundary_vertex_dual_polygon(&self, owner_id: ChunkId, vertex_idx: HexPointyDir) -> Option<Vec<Vec2>> {
        let v0 = vertex_idx;
        let (n1, v1, n2, v2) = match vertex_idx {
            HexPointyDir::E => (HexFlatDir::SE, HexPointyDir::NW, HexFlatDir::NE, HexPointyDir::SW),
            HexPointyDir::NE => (HexFlatDir::NE, HexPointyDir::W, HexFlatDir::N, HexPointyDir::SE),
            HexPointyDir::NW => (HexFlatDir::N, HexPointyDir::SW, HexFlatDir::NW, HexPointyDir::E),
            HexPointyDir::W => (HexFlatDir::NW, HexPointyDir::SE, HexFlatDir::SW, HexPointyDir::NE),
            HexPointyDir::SW => (HexFlatDir::SW, HexPointyDir::E, HexFlatDir::S, HexPointyDir::NW),
            HexPointyDir::SE => (HexFlatDir::S, HexPointyDir::NE, HexFlatDir::SE, HexPointyDir::W),
        };

        let id0 = owner_id;
        let id1 = owner_id.neighbor(n1);
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
