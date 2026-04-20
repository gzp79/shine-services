use crate::{
    indexed::IdxVec,
    math::{
        prng::SplitMix64,
        quadrangulation::{QuadIdx, QuadTopology, VertIdx},
    },
    world::{Chunk, ChunkId},
};
use glam::Vec2;
use std::collections::HashMap;

/// The core subdivision depth to align chunks
pub const SUBDIVISION_BASE: u32 = 4;
/// The numbe of cells on the edge of a chunk
pub const SUBDIVISION_COUNT: u32 = 2u32.pow(SUBDIVISION_BASE);

/// The world size of a chunk (in meter)
pub const CHUNK_WORLD_SIZE: f32 = 1000.0;
/// The "ideal" length of the side of a cell (in meter)
pub const CELL_WORLD_SIZE: f32 = CHUNK_WORLD_SIZE / SUBDIVISION_COUNT as f32;

/// Merge vertex rings from two chunks, replacing ghost quads with real data from neighbor.
/// Returns CCW-ordered quad centers forming the complete dual cell polygon.
///
/// Note: This is the critical piece - ring alignment logic needs careful implementation.
/// Ghost quads in one chunk correspond to real quads in the neighbor.
pub(crate) fn merge_vertex_ring_dual(
    topo1: &QuadTopology,
    centers1: &IdxVec<QuadIdx, Vec2>,
    v1: VertIdx,
    topo2: &QuadTopology,
    centers2: &IdxVec<QuadIdx, Vec2>,
    v2: VertIdx,
) -> Vec<Vec2> {
    // Collect rings with ghost markers
    let ring1: Vec<Option<Vec2>> = topo1
        .vertex_ring_ccw(v1)
        .map(|qv| {
            if topo1.is_ghost_quad(qv.quad) {
                None
            } else {
                Some(centers1[qv.quad])
            }
        })
        .collect();

    let ring2: Vec<Option<Vec2>> = topo2
        .vertex_ring_ccw(v2)
        .map(|qv| {
            if topo2.is_ghost_quad(qv.quad) {
                None
            } else {
                Some(centers2[qv.quad])
            }
        })
        .collect();

    // Find where ghost quad sequence starts in each ring
    let ghost_start1 = ring1.iter().position(|opt| opt.is_none());
    let ghost_start2 = ring2.iter().position(|opt| opt.is_none());

    // Build merged ring by taking real quads from both
    let mut merged = Vec::new();

    if let (Some(gs1), Some(gs2)) = (ghost_start1, ghost_start2) {
        // Add real quads from ring1 before ghost sequence
        for opt in ring1.iter().take(gs1) {
            if let Some(pos) = opt {
                merged.push(*pos);
            }
        }

        // Add real quads from ring2 before its ghost sequence
        // These fill the gap left by ring1's ghosts
        for opt in ring2.iter().take(gs2) {
            if let Some(pos) = opt {
                merged.push(*pos);
            }
        }
    } else {
        // No ghost quads found - just use ring1 (interior vertex)
        for opt in ring1.iter() {
            if let Some(pos) = opt {
                merged.push(*pos);
            }
        }
    }

    merged
}

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
    pub fn boundary_edge_dual_polygons(
        &self,
        owner_id: ChunkId,
        edge_idx: u8,
    ) -> Option<(Vec<f32>, Vec<u32>, Vec<u32>)> {
        let owner = self.chunk(owner_id)?;
        let neighbor_id = owner_id.neighbor(edge_idx);
        let neighbor = self.chunk(neighbor_id)?;

        let owner_verts = owner.boundary_edge_vertices(edge_idx);
        let neighbor_edge_idx = (edge_idx + 3) % 6; // Opposite edge
        let neighbor_verts = neighbor.boundary_edge_vertices(neighbor_edge_idx);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut starts = vec![0];

        for i in 0..owner_verts.len() {
            // Neighbor vertices are in reverse order (opposite direction)
            let owner_vi = owner_verts[i];
            let neighbor_vi = neighbor_verts[neighbor_verts.len() - 1 - i];

            // Merge vertex rings from both chunks
            let polygon = merge_vertex_ring_dual(
                &owner.topology,
                &owner.quad_centers,
                owner_vi,
                &neighbor.topology,
                &neighbor.quad_centers,
                neighbor_vi,
            );

            if polygon.len() >= 3 {
                let start_idx = (vertices.len() / 2) as u32;
                for (j, pos) in polygon.iter().enumerate() {
                    vertices.push(pos.x);
                    vertices.push(pos.y);
                    indices.push(start_idx + j as u32);
                }
                starts.push(indices.len() as u32);
            } else {
                // Degenerate polygon - skip it
                starts.push(indices.len() as u32);
            }
        }

        Some((vertices, indices, starts))
    }

    /// Returns dual polygon for boundary vertex cell (single triangular cell).
    /// TODO: Implement full 3-chunk merge
    pub fn boundary_vertex_dual_polygon(
        &self,
        _owner_id: ChunkId,
        _vertex_idx: u8,
    ) -> Option<(Vec<f32>, Vec<u32>, Vec<u32>)> {
        // Stub: return empty for now
        None
    }
}
